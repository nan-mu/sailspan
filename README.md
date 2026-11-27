# Sailspan

Sailspan disassembles an original little-endian ELF64 eBPF object into an
Instruction Table and partitions its instructions into Nodes and Boundaries.
The partition includes unreachable instructions and records static Candidate
Targets without evaluating branches.

## Generate the firewall artifacts

The release run used Rust/Cargo 1.100.0, p7zip 17.05 as `7z` on PATH,
and Homebrew LLVM 23.1.1 for comparison. Download the pinned release asset,
then run Sailspan using a fresh output directory.

```sh
curl -fL -o firewall.7z https://github.com/nan-mu/libbpf-mcu/releases/download/v0.1.2-rc.1/firewall.7z
cargo run --locked -- firewall firewall.7z firewall-output
cargo run --locked -- verify firewall-output
```

The `firewall` command checks the archive SHA-256, locates exactly one
`objects/firewall_bpf.o` member, and checks its size and SHA-256. It publishes
both schema v2 JSON files in the output directory.

- `disassembly.json` holds the source identity and digest, nonempty executable
  sections, function entries, and the numbered Instruction Table. Retrieve
  an instruction's bytes from the matching object starting at
  `section.file_offset + 8 * instruction.slot`, with length
  `8 * instruction.slot_count`. LDDW occupies two slots under one ID.
- `nodes.json` holds the SHA-256 of the exact Instruction Table file bytes
  and Nodes and Boundaries that refer to its Instruction IDs. An unresolved
  Candidate Target carries a diagnostic on its Boundary.

The `verify` command reads both files as Rust types and checks their association,
versions, partition, and references. The library exposes the same pipeline through
`ReleaseInput::extract`, `disassemble`, `generate_nodes`, `write_artifacts`, and
`read_artifacts`. The decoder supports the instruction families needed by the
pinned object and the structural tests. It reports unsupported encodings with
their section and slot.

Run the synthetic contract tests with `cargo test --locked`.
