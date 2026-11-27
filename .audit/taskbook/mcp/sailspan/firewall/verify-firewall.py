"""Compare the fixed firewall artifacts with independent LLVM output."""
import hashlib
import json
import re
import sys
from pathlib import Path

root, artifacts = map(Path, sys.argv[1:])
elf = (root / "firewall_bpf.o").read_bytes()
headers = (root / "readelf.txt").read_text()
disassembly = (root / "objdump.txt").read_text()
table_bytes = (artifacts / "disassembly.json").read_bytes()
table = json.loads(table_bytes)
nodes = json.loads((artifacts / "nodes.json").read_bytes())

assert "EM_BPF" in headers and "REL (Relocatable file)" in headers and "little endian" in headers
section_pattern = r"^\s*\[\s*(\d+)\]\s+(\S+)\s+PROGBITS\s+\S+\s+(\S+)\s+(\S+)\s+\S+\s+AX\b"
sections = [(int(i), name, int(offset, 16), int(size, 16))
            for i, name, offset, size in re.findall(section_pattern, headers, re.M)
            if int(size, 16)]
assert len(sections) == 1
section_index, section_name, file_offset, byte_count = sections[0]
section, = table["sections"]
assert (section["index"], section["name"], section["file_offset"], section["slot_count"] * 8) == sections[0]
entry = re.search(r"^\s*(\d+):\s+(\w+)\s+\d+\s+FUNC\s+GLOBAL\s+DEFAULT\s+(\d+)\s+(firewall)$", headers, re.M)
symbol_index, address, entry_section = int(entry[1]), int(entry[2], 16), int(entry[3])
assert section["entries"] == [{"slot": address // 8,
    "symbol": {"index": symbol_index, "name": entry[4], "value": address, "section_index": entry_section}}]
assert table["object_sha256"] == hashlib.sha256(elf).hexdigest()
assert table["source"] == {"kind": "release", "tag": "v0.1.2-rc.1", "asset": "firewall.7z",
                           "member": "objects/firewall_bpf.o"}
assert nodes["instruction_table_sha256"] == hashlib.sha256(table_bytes).hexdigest()

llvm = []
for line in disassembly.splitlines():
    match = re.match(r"^\s*(\d+):\s+((?:[\da-f]{2}[ \t]+){8,16})(.+)$", line)
    if match:
        llvm.append((int(match[1]), bytes.fromhex(match[2]), match[3].strip()))
instructions = table["instructions"]
assert len(llvm) == len(instructions)
id_at = {slot: index for index, (slot, _, _) in enumerate(llvm)}
starts = {id_at[address // 8]}
expected_boundaries = []

def normalize(assembly):
    assembly = re.sub(r"\s+<[^>]+>$", "", assembly)
    assembly = re.sub(r"0x[0-9a-f]+", lambda m: str(int(m[0], 16)), assembly)
    return re.sub(r"\s+", "", assembly)

covered_slots = []
for index, ((slot, encoded, assembly), insn) in enumerate(zip(llvm, instructions)):
    assert (index, section_index, slot, len(encoded) // 8) == (
        insn["id"], insn["section_index"], insn["slot"], insn["slot_count"])
    start = file_offset + slot * 8
    assert elf[start:start + len(encoded)] == encoded
    assert normalize(assembly) == normalize(insn["assembly"]), (slot, assembly, insn["assembly"])
    covered_slots.extend(range(slot, slot + len(encoded) // 8))
    targets = []
    fallthrough = None
    if assembly.startswith("if "):
        kind = "conditional_jump"
        delta = re.search(r" goto ([+-](?:0x[\da-f]+|\d+))", assembly)
        targets = [id_at[slot + 1 + int(delta[1], 0)]]
        fallthrough = id_at[slot + 1]
    elif assembly.startswith("call "):
        kind = "helper"
        fallthrough = id_at[slot + 1]
    elif assembly == "exit":
        kind = "exit"
    else:
        continue
    starts.update(targets)
    expected_boundaries.append({"id": f"b{len(expected_boundaries)}", "kind": kind,
        "instruction_ids": [index], "targets": targets, "fallthrough": fallthrough, "unresolved": None})
assert covered_slots == list(range(byte_count // 8))
assert nodes["boundaries"] == expected_boundaries

boundary_ids = {b["instruction_ids"][0] for b in expected_boundaries}
expected_nodes = []
for index in range(len(llvm)):
    if index in boundary_ids:
        continue
    if index == 0 or index in starts or index - 1 in boundary_ids:
        expected_nodes.append({"id": f"n{len(expected_nodes)}", "instruction_ids": []})
    expected_nodes[-1]["instruction_ids"].append(index)
assert nodes["nodes"] == expected_nodes

for insn in instructions:
    begin = insn["slot"] * 8
    end = begin + insn["slot_count"] * 8
    assert all(begin <= r["byte_offset"] < end for r in insn["relocations"])
relocations = [r for insn in instructions for r in insn["relocations"]]
core = re.findall(r"^\s*([0-9a-f]+):\s+CO-RE <byte_off> \[(\d+)\] .*? \(([\d:]+)\)$", disassembly, re.M)
assert [(r["byte_offset"], r["kind"], r["type_id"], r["access_string"])
        for r in relocations if r["format"] == "core"] == [
            (int(offset, 16), 0, int(type_id), access) for offset, type_id, access in core]
elf_relocation = re.search(r"^([\da-f]+)\s+([\da-f]+)\s+R_BPF_64_64\s+([\da-f]+)\s+(\w+)$", headers, re.M)
offset, info, value, symbol = elf_relocation.groups()
offset, info, value = (int(field, 16) for field in (offset, info, value))
symbol_section = int(re.search(rf"^\s*{info >> 32}:.*?\s+(\d+)\s+{symbol}$", headers, re.M)[1])
addend = int.from_bytes(elf[file_offset + offset + 4:file_offset + offset + 8], "little")
assert [r for r in relocations if r["format"] == "elf"] == [
    {"format": "elf", "byte_offset": offset, "elf_type": info & 0xffffffff,
     "symbol": {"index": info >> 32, "name": symbol, "section_index": symbol_section, "value": value},
     "addend": {"kind": "implicit", "value": addend}}]
assert len(relocations) == len(core) + 1
print(f"LLVM matched {len(llvm)} instructions and {len(covered_slots)} slots in {section_name}, "
      f"{len(expected_nodes)} nodes, {len(expected_boundaries)} boundaries and {len(relocations)} relocations")
print("Node slots:", [[llvm[i][0] for i in node["instruction_ids"]] for node in expected_nodes])
print("Boundary (slot, candidate slots, fallthrough slot):", [
    (llvm[b["instruction_ids"][0]][0], [llvm[i][0] for i in b["targets"]],
     None if b["fallthrough"] is None else llvm[b["fallthrough"]][0]) for b in expected_boundaries])
