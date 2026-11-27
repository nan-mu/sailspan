use object::write::Object;
use object::{Architecture, BinaryFormat, Endianness, SectionKind};

fn example(jump_offset: u8) -> Vec<u8> {
    let mut elf = Object::new(BinaryFormat::Elf, Architecture::Bpf, Endianness::Little);
    let section = elf.add_section(Vec::new(), b"xdp".to_vec(), SectionKind::Text);
    elf.append_section_data(
        section,
        &[
            [0xbf, 0x12, 0, 0, 0, 0, 0, 0],           // r2 = r1
            [0x07, 0x02, 0, 0, 4, 0, 0, 0],           // r2 += 4
            [0x2d, 0x32, jump_offset, 0, 0, 0, 0, 0], // conditional jump
            [0x18, 0x01, 0, 0, 0, 0, 0, 0],           // r1 = 4294967296 ll
            [0, 0, 0, 0, 1, 0, 0, 0],                 // LDDW continuation
            [0xb7, 0, 0, 0, 2, 0, 0, 0],              // r0 = 2
            [0x85, 0, 0, 0, 1, 0, 0, 0],              // call 1
            [0x95, 0, 0, 0, 0, 0, 0, 0],              // exit
        ]
        .concat(),
        8,
    );
    elf.write().unwrap()
}

#[test]
fn jump_slot_resolves_to_instruction_id_across_a_wide_instruction() {
    let table = sailspan::disassemble(&example(2), sailspan::Source::Synthetic).unwrap();
    let directory = tempfile::tempdir().unwrap();
    let output = directory.path().join("artifacts");
    let nodes = sailspan::write_artifacts(&output, &table).unwrap();
    assert_eq!(
        sailspan::read_artifacts(&output).unwrap(),
        (table.clone(), nodes.clone())
    );

    assert_eq!(table.instructions[3].slot, 3);
    assert_eq!(table.instructions[3].slot_count, 2);
    assert_eq!(table.instructions[3].assembly, "r1 = 4294967296 ll");
    assert_eq!(table.instructions[4].slot, 5);
    assert_eq!(nodes.boundaries[0].targets, [4]);
    assert_eq!(nodes.boundaries[0].fallthrough, Some(3));
    assert_eq!(
        nodes
            .nodes
            .iter()
            .map(|node| &node.instruction_ids)
            .collect::<Vec<_>>(),
        [&vec![0, 1], &vec![3], &vec![4]]
    );
}

#[test]
fn jump_into_lddw_continuation_is_reported_without_inventing_a_target() {
    let table = sailspan::disassemble(&example(1), sailspan::Source::Synthetic).unwrap();
    let artifact = sailspan::generate_nodes(&table).unwrap();
    let jump = &artifact.boundaries[0];
    assert!(jump.targets.is_empty());
    assert_eq!(jump.fallthrough, Some(3));
    assert!(jump.unresolved.as_deref().unwrap().contains("slot 4"));
}

#[test]
fn readback_rejects_tampered_tables_and_invalid_ownership() {
    let table = sailspan::disassemble(&example(2), sailspan::Source::Synthetic).unwrap();
    let directory = tempfile::tempdir().unwrap();
    let output = directory.path().join("artifacts");
    let mut artifact = sailspan::write_artifacts(&output, &table).unwrap();
    let table_file = output.join("disassembly.json");
    let original = std::fs::read(&table_file).unwrap();
    let mut tampered = original.clone();
    tampered.push(b' ');
    std::fs::write(&table_file, tampered).unwrap();
    assert!(
        sailspan::read_artifacts(&output)
            .unwrap_err()
            .to_string()
            .contains("digest mismatch")
    );
    std::fs::write(&table_file, original).unwrap();

    let node_file = output.join("nodes.json");
    artifact.nodes[0].instruction_ids[1] = 99;
    std::fs::write(&node_file, serde_json::to_vec(&artifact).unwrap()).unwrap();
    assert!(sailspan::read_artifacts(&output).is_err());

    artifact.nodes[0].instruction_ids[1] = 3;
    std::fs::write(&node_file, serde_json::to_vec(&artifact).unwrap()).unwrap();
    assert!(sailspan::read_artifacts(&output).is_err());
}

#[test]
fn jmp32_uses_its_immediate_and_resolves_only_within_its_section() {
    let mut elf = Object::new(BinaryFormat::Elf, Architecture::Bpf, Endianness::Little);
    let first = elf.add_section(Vec::new(), b"xdp".to_vec(), SectionKind::Text);
    let second = elf.add_section(Vec::new(), b".text".to_vec(), SectionKind::Text);
    let exit = [0x95, 0, 0, 0, 0, 0, 0, 0];
    elf.append_section_data(first, &exit, 8);
    elf.append_section_data(
        second,
        &[
            [0x06, 0, 0, 0, 1, 0, 0, 0], // goto32 +1 from slot 0
            [0xb7, 0, 0, 0, 2, 0, 0, 0],
            exit,
        ]
        .concat(),
        8,
    );
    let table = sailspan::disassemble(&elf.write().unwrap(), sailspan::Source::Synthetic).unwrap();
    let nodes = sailspan::generate_nodes(&table).unwrap();
    assert_eq!(table.instructions[0].slot, table.instructions[1].slot);
    assert_ne!(
        table.instructions[0].section_index,
        table.instructions[1].section_index
    );
    assert_eq!(nodes.boundaries[1].instruction_ids, [1]);
    assert_eq!(nodes.boundaries[1].targets, [3]);
    assert_eq!(nodes.boundaries[1].fallthrough, None);
    assert_eq!(nodes.nodes[0].instruction_ids, [2]); // Unreachable, but still owned.
}
