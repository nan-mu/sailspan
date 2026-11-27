use crate::artifact::{SLOT_BYTES, VERSION};
use crate::{
    Addend, Entry, Instruction, InstructionTable, ProgramSection, Relocation, Source, Symbol, ebpf,
    sha256,
};
use anyhow::{Context, Result, bail, ensure};
use object::{
    Architecture, Object, ObjectKind, ObjectSection, ObjectSymbol, RelocationFlags,
    RelocationTarget, SectionFlags, SymbolKind,
};
use std::collections::BTreeMap;

/// Read and disassemble an original little-endian ELF64 eBPF relocatable object.
pub fn disassemble(bytes: &[u8], source: Source) -> Result<InstructionTable> {
    let file = object::read::elf::ElfFile64::<object::Endianness>::parse(bytes)
        .context("parse ELF64 object")?;
    ensure!(
        file.architecture() == Architecture::Bpf && file.kind() == ObjectKind::Relocatable,
        "expected a relocatable eBPF object"
    );
    ensure!(file.is_little_endian(), "big-endian eBPF is unsupported");

    let mut executable = file.sections().filter(|section| {
        matches!(section.flags(), SectionFlags::Elf { sh_flags } if sh_flags & u64::from(object::elf::SHF_EXECINSTR) != 0)
            && section.size() > 0
    }).collect::<Vec<_>>();
    executable.sort_by_key(|section| section.index().0);
    ensure!(!executable.is_empty(), "no nonempty executable sections");

    let mut core = core_relocations(&file)?;
    let mut sections = Vec::new();
    let mut instructions = Vec::new();
    for section in executable {
        let index = section.index().0;
        let name = section.name()?.to_owned();
        let data = section.data()?;
        let (file_offset, _) = section
            .file_range()
            .context("executable section is not file-backed")?;
        ensure!(
            data.len() as u64 == section.size() && section.size() % SLOT_BYTES == 0,
            "invalid instruction bytes in section {name}"
        );
        let mut relocations = core.remove(&name).unwrap_or_default();
        for (offset, relocation) in section.relocations() {
            relocations.push(elf_relocation(&file, data, offset, relocation)?);
        }
        relocations.sort_by_key(Relocation::byte_offset);
        let mut relocations = relocations.into_iter().peekable();

        let mut start = 0;
        while start < data.len() {
            let slot = start as u64 / SLOT_BYTES;
            let (slot_count, mut assembly, flow) = ebpf::decode(&data[start..])
                .with_context(|| format!("decode section {name} slot {slot}"))?;
            let end = start + usize::from(slot_count) * SLOT_BYTES as usize;
            let mut attached = Vec::new();
            while let Some(relocation) = relocations.next_if(|r| r.byte_offset() < end as u64) {
                if let Relocation::Elf {
                    symbol:
                        Some(Symbol {
                            name: Some(symbol), ..
                        }),
                    ..
                } = &relocation
                {
                    assembly.push_str(&format!(" <{symbol}>"));
                }
                attached.push(relocation);
            }
            instructions.push(Instruction {
                id: instructions.len(),
                section_index: index,
                slot,
                slot_count,
                assembly,
                flow,
                relocations: attached,
            });
            start = end;
        }
        ensure!(
            relocations.next().is_none(),
            "relocation outside section {name}"
        );

        let mut entries = Vec::new();
        for symbol in file.symbols().filter(|symbol| {
            symbol.kind() == SymbolKind::Text && symbol.section_index() == Some(section.index())
        }) {
            ensure!(
                symbol.address() % SLOT_BYTES == 0,
                "unaligned function symbol in {name}"
            );
            entries.push(Entry {
                slot: symbol.address() / SLOT_BYTES,
                symbol: describe_symbol(&file, &symbol)?,
            });
        }
        entries.sort_by_key(|entry| (entry.slot, entry.symbol.index));
        sections.push(ProgramSection {
            index,
            name,
            file_offset,
            slot_count: section.size() / SLOT_BYTES,
            entries,
        });
    }
    ensure!(
        core.is_empty(),
        "CO-RE relocation references an unknown or empty executable section"
    );

    Ok(InstructionTable {
        schema_version: VERSION,
        architecture: "ebpf".into(),
        endianness: "little".into(),
        source,
        object_sha256: sha256(bytes),
        sections,
        instructions,
    })
}

fn describe_symbol<'data>(
    file: &impl Object<'data>,
    symbol: &impl ObjectSymbol<'data>,
) -> Result<Symbol> {
    let section_index = symbol.section_index();
    let name = if !symbol.name()?.is_empty() {
        Some(symbol.name()?.to_owned())
    } else if symbol.kind() == SymbolKind::Section {
        section_index
            .map(|index| -> Result<String> { Ok(file.section_by_index(index)?.name()?.to_owned()) })
            .transpose()?
    } else {
        None
    };
    Ok(Symbol {
        index: symbol.index().0,
        name,
        section_index: section_index.map(|index| index.0),
        value: symbol.address(),
    })
}

fn elf_relocation<'data>(
    file: &impl Object<'data>,
    data: &[u8],
    byte_offset: u64,
    relocation: object::Relocation,
) -> Result<Relocation> {
    let RelocationFlags::Elf { r_type: elf_type } = relocation.flags() else {
        bail!("expected an ELF relocation");
    };
    let symbol = match relocation.target() {
        RelocationTarget::Symbol(index) => {
            Some(describe_symbol(file, &file.symbol_by_index(index)?)?)
        }
        RelocationTarget::Absolute => None,
        other => bail!("unsupported ELF relocation target {other:?}"),
    };
    let addend = if !relocation.has_implicit_addend() {
        Addend::Explicit {
            value: relocation.addend(),
        }
    } else if matches!(
        elf_type,
        object::elf::R_BPF_64_64 | object::elf::R_BPF_64_32
    ) {
        let start = usize::try_from(byte_offset)?;
        let first = data
            .get(start..)
            .and_then(|tail| tail.get(..8))
            .context("relocation outside section")?;
        let encoded = first[4..8].try_into()?;
        let value = if elf_type == object::elf::R_BPF_64_64 {
            ensure!(first[0] == 0x18, "R_BPF_64_64 must refer to LDDW");
            i64::from(u32::from_le_bytes(encoded))
        } else {
            ensure!(first[0] == 0x85, "R_BPF_64_32 must refer to a call");
            (i64::from(i32::from_le_bytes(encoded)) + 1) * SLOT_BYTES as i64
        };
        Addend::Implicit { value }
    } else {
        Addend::Unknown
    };
    Ok(Relocation::Elf {
        byte_offset,
        elf_type,
        symbol,
        addend,
    })
}

// CO-RE records reference strings by offset in .BTF and instructions by byte offset
// in the named executable section. The original type ID and access path are retained.
fn core_relocations<'data>(file: &impl Object<'data>) -> Result<BTreeMap<String, Vec<Relocation>>> {
    let mut result = BTreeMap::<String, Vec<Relocation>>::new();
    let Some(extension) = file.section_by_name(".BTF.ext") else {
        return Ok(result);
    };
    let extension = extension.data()?;
    let header = btf_header_size(extension)?;
    if header == 24 {
        return Ok(result); // The original header predated the CO-RE subsection.
    }
    ensure!(header >= 32, "invalid .BTF.ext header length");
    let mut records = subsection(extension, header, 24)?;
    if records.is_empty() {
        return Ok(result);
    }

    let btf = file
        .section_by_name(".BTF")
        .context("CO-RE records require .BTF strings")?
        .data()?;
    let btf_header = btf_header_size(btf)?;
    ensure!(btf_header >= 24, "invalid .BTF header length");
    let strings = subsection(btf, btf_header, 16)?;
    let record_size = word(records, 0)? as usize;
    ensure!(record_size >= 16, "invalid CO-RE record size");
    records = &records[4..];
    while !records.is_empty() {
        let name_offset = word(records, 0)?;
        let count = word(records, 4)? as usize;
        let length = count
            .checked_mul(record_size)
            .context("CO-RE record count overflow")?;
        let (group, rest) = records[8..]
            .split_at_checked(length)
            .context("truncated CO-RE records")?;
        records = rest;
        let section = btf_string(strings, name_offset)?.to_owned();
        let entries = result.entry(section).or_default();
        for record in group.chunks_exact(record_size) {
            let access_string_offset = word(record, 8)?;
            entries.push(Relocation::Core {
                byte_offset: u64::from(word(record, 0)?),
                kind: word(record, 12)?,
                type_id: word(record, 4)?,
                access_string_offset,
                access_string: btf_string(strings, access_string_offset)?.to_owned(),
            });
        }
    }
    Ok(result)
}

fn btf_header_size(bytes: &[u8]) -> Result<usize> {
    ensure!(
        bytes.starts_with(&[0x9f, 0xeb, 1]),
        "invalid BTF magic or version"
    );
    let header = word(bytes, 4)? as usize;
    ensure!(header <= bytes.len(), "truncated BTF header");
    Ok(header)
}

fn word(bytes: &[u8], offset: usize) -> Result<u32> {
    let field = bytes
        .get(offset..offset + 4)
        .context("truncated BTF field")?;
    Ok(u32::from_le_bytes(field.try_into()?))
}

fn subsection(bytes: &[u8], header: usize, field: usize) -> Result<&[u8]> {
    let offset = word(bytes, field)? as usize;
    let length = word(bytes, field + 4)? as usize;
    bytes[header..]
        .get(offset..)
        .and_then(|tail| tail.get(..length))
        .context("BTF subsection outside section")
}

fn btf_string(strings: &[u8], offset: u32) -> Result<&str> {
    let tail = strings
        .get(offset as usize..)
        .context("BTF string offset outside table")?;
    let string = std::ffi::CStr::from_bytes_until_nul(tail).context("unterminated BTF string")?;
    Ok(string.to_str()?)
}
