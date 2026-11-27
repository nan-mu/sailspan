mod artifact;
mod ebpf;
mod elf;
mod nodes;
mod release;

pub use artifact::{
    Addend, Boundary, BoundaryKind, Entry, Flow, Instruction, InstructionTable, Node, NodeArtifact,
    ProgramSection, Relocation, Source, Symbol, read_artifacts, write_artifacts,
};
pub use elf::disassemble;
pub use nodes::generate_nodes;
pub use release::{FIREWALL, ReleaseInput};

fn sha256(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    format!("{:x}", Sha256::digest(bytes))
}
