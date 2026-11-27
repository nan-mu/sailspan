use crate::artifact::{SLOT_BYTES, VERSION};
use crate::{
    Addend, Boundary, BoundaryKind, Flow, Instruction, InstructionTable, Node, NodeArtifact,
    Relocation, Source, sha256,
};
use anyhow::{Context, Result, ensure};
use std::collections::{BTreeMap, BTreeSet};

type Location = (usize, u64);
type Locations = BTreeMap<Location, usize>;

/// Partition a validated instruction table without evaluating program behavior.
pub fn generate_nodes(table: &InstructionTable) -> Result<NodeArtifact> {
    let locations = index_instructions(table)?;
    let mut starts = BTreeSet::new();
    for section in &table.sections {
        for entry in &section.entries {
            starts.insert(locations[&(section.index, entry.slot)]);
        }
    }

    let mut boundaries = Vec::new();
    for instruction in &table.instructions {
        if let Some(flow) = &instruction.flow {
            let boundary = classify(instruction, flow, &locations, boundaries.len());
            starts.extend(boundary.targets.iter().copied());
            boundaries.push(boundary);
        }
    }

    let mut nodes: Vec<Node> = Vec::new();
    let mut previous = None;
    for instruction in &table.instructions {
        if instruction.flow.is_some() {
            previous = None;
            continue;
        }
        if previous != Some(instruction.section_index) || starts.contains(&instruction.id) {
            nodes.push(Node {
                id: format!("n{}", nodes.len()),
                instruction_ids: Vec::new(),
            });
        }
        nodes
            .last_mut()
            .expect("ordinary instruction starts or extends a node")
            .instruction_ids
            .push(instruction.id);
        previous = Some(instruction.section_index);
    }

    Ok(NodeArtifact {
        schema_version: VERSION,
        instruction_table_sha256: sha256(&table.json()?),
        nodes,
        boundaries,
    })
}

fn index_instructions(table: &InstructionTable) -> Result<Locations> {
    ensure!(
        table.schema_version == VERSION,
        "unsupported instruction table version"
    );
    ensure!(
        table.architecture == "ebpf" && table.endianness == "little",
        "unsupported architecture or endianness"
    );
    ensure!(
        table.object_sha256.len() == 64
            && table.object_sha256.bytes().all(|b| b.is_ascii_hexdigit()),
        "invalid object SHA-256"
    );
    if let Source::Release { tag, asset, member } = &table.source {
        ensure!(
            !tag.is_empty() && !asset.is_empty() && !member.is_empty(),
            "incomplete release identity"
        );
    }
    ensure!(!table.sections.is_empty(), "no executable instructions");

    let mut locations = Locations::new();
    let mut instructions = table.instructions.iter().enumerate().peekable();
    let mut previous_section = 0;
    for section in &table.sections {
        ensure!(
            section.index > previous_section && section.slot_count > 0,
            "invalid executable section order or size"
        );
        previous_section = section.index;
        let mut slot = 0;
        while let Some((id, instruction)) =
            instructions.next_if(|(_, i)| i.section_index == section.index)
        {
            let width = u64::from(instruction.slot_count);
            ensure!(
                instruction.id == id
                    && instruction.slot == slot
                    && (1..=2).contains(&width)
                    && (width == 1 || instruction.flow.is_none())
                    && !instruction.assembly.is_empty(),
                "invalid instruction {id} in section {}",
                section.index
            );
            let end_slot = slot
                .checked_add(width)
                .context("instruction slot overflow")?;
            ensure!(
                end_slot <= section.slot_count && end_slot <= u64::MAX / SLOT_BYTES,
                "instruction outside section"
            );
            for relocation in &instruction.relocations {
                ensure!(
                    (slot * SLOT_BYTES..end_slot * SLOT_BYTES).contains(&relocation.byte_offset()),
                    "relocation outside instruction {id}"
                );
            }
            locations.insert((section.index, slot), id);
            slot = end_slot;
        }
        ensure!(
            slot == section.slot_count,
            "incomplete instruction coverage in section {}",
            section.index
        );
        for entry in &section.entries {
            ensure!(
                entry.symbol.section_index == Some(section.index)
                    && entry.symbol.value
                        == entry
                            .slot
                            .checked_mul(SLOT_BYTES)
                            .context("entry slot overflow")?
                    && locations.contains_key(&(section.index, entry.slot)),
                "invalid entry in section {} at slot {}",
                section.index,
                entry.slot
            );
        }
    }
    ensure!(
        instructions.next().is_none(),
        "instructions outside ordered executable sections"
    );
    Ok(locations)
}

fn classify(
    instruction: &Instruction,
    flow: &Flow,
    locations: &Locations,
    index: usize,
) -> Boundary {
    use BoundaryKind::*;
    let next = instruction.slot + 1;
    let location = |slot| locations.get(&(instruction.section_index, slot)).copied();
    let mut boundary = Boundary {
        id: format!("b{index}"),
        kind: Exit,
        instruction_ids: vec![instruction.id],
        targets: Vec::new(),
        fallthrough: None,
        unresolved: None,
    };

    match *flow {
        Flow::Exit => {}
        Flow::Jump {
            offset,
            conditional,
        } => {
            boundary.kind = if conditional {
                ConditionalJump
            } else {
                UnconditionalJump
            };
            let slot = next.checked_add_signed(i64::from(offset));
            if let Some(id) = slot.and_then(location) {
                boundary.targets.push(id);
            } else {
                boundary.unresolved = Some(match slot {
                    Some(slot) => format!("jump target slot {slot} is not an instruction start"),
                    None => format!(
                        "jump from slot {} with offset {offset} is outside the section",
                        instruction.slot
                    ),
                });
            }
        }
        Flow::Call { mode: 0, .. } => boundary.kind = Helper,
        Flow::Call { mode: 1, immediate } => {
            boundary.kind = LocalCall;
            if let Some(id) = local_call_target(instruction, immediate, locations) {
                boundary.targets.push(id);
            } else {
                boundary.unresolved =
                    Some("local call target is unavailable or is not an instruction start".into());
            }
        }
        Flow::Call { mode: 2, .. } => {
            boundary.kind = KernelCall;
            boundary.unresolved =
                Some("kernel function target requires external resolution".into());
        }
        Flow::Call { mode, .. } => {
            boundary.kind = UnsupportedCall;
            boundary.unresolved = Some(format!("unsupported call source {mode}"));
        }
        Flow::IndirectCall => {
            boundary.kind = UnsupportedCall;
            boundary.unresolved = Some("indirect call target is dynamic".into());
        }
    }
    if !matches!(boundary.kind, Exit | UnconditionalJump) {
        boundary.fallthrough = location(next);
    }
    boundary
}

fn local_call_target(
    instruction: &Instruction,
    immediate: i32,
    locations: &Locations,
) -> Option<usize> {
    for relocation in &instruction.relocations {
        if let Relocation::Elf {
            elf_type: object::elf::R_BPF_64_32,
            symbol,
            addend,
            ..
        } = relocation
        {
            let symbol = symbol.as_ref()?;
            let (Addend::Implicit { value } | Addend::Explicit { value }) = addend else {
                return None;
            };
            let byte_offset = symbol.value.checked_add_signed(*value)?;
            if byte_offset % SLOT_BYTES != 0 {
                return None;
            }
            return locations
                .get(&(symbol.section_index?, byte_offset / SLOT_BYTES))
                .copied();
        }
    }
    let slot = (instruction.slot + 1).checked_add_signed(i64::from(immediate))?;
    locations.get(&(instruction.section_index, slot)).copied()
}
