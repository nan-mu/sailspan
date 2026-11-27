use crate::{generate_nodes, sha256};
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use std::{fs, path::Path};

pub(crate) const VERSION: u32 = 2;
pub(crate) const SLOT_BYTES: u64 = 8;
const TABLE_FILE: &str = "disassembly.json";
const NODE_FILE: &str = "nodes.json";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Source {
    Synthetic,
    Release {
        tag: String,
        asset: String,
        member: String,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InstructionTable {
    pub schema_version: u32,
    pub architecture: String,
    pub endianness: String,
    pub source: Source,
    pub object_sha256: String,
    pub sections: Vec<ProgramSection>,
    pub instructions: Vec<Instruction>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProgramSection {
    pub index: usize,
    pub name: String,
    pub file_offset: u64,
    pub slot_count: u64,
    pub entries: Vec<Entry>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Entry {
    pub slot: u64,
    pub symbol: Symbol,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Symbol {
    pub index: usize,
    pub name: Option<String>,
    pub section_index: Option<usize>,
    pub value: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Addend {
    Explicit { value: i64 },
    Implicit { value: i64 },
    Unknown,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "format", rename_all = "snake_case", deny_unknown_fields)]
pub enum Relocation {
    Elf {
        byte_offset: u64,
        elf_type: u32,
        symbol: Option<Symbol>,
        addend: Addend,
    },
    Core {
        byte_offset: u64,
        kind: u32,
        type_id: u32,
        access_string_offset: u32,
        access_string: String,
    },
}

impl Relocation {
    pub(crate) fn byte_offset(&self) -> u64 {
        match self {
            Self::Elf { byte_offset, .. } | Self::Core { byte_offset, .. } => *byte_offset,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Flow {
    Jump { offset: i32, conditional: bool },
    Call { mode: u8, immediate: i32 },
    IndirectCall,
    Exit,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Instruction {
    pub id: usize,
    pub section_index: usize,
    pub slot: u64,
    pub slot_count: u8,
    pub assembly: String,
    pub flow: Option<Flow>,
    pub relocations: Vec<Relocation>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NodeArtifact {
    pub schema_version: u32,
    pub instruction_table_sha256: String,
    pub nodes: Vec<Node>,
    pub boundaries: Vec<Boundary>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Node {
    pub id: String,
    pub instruction_ids: Vec<usize>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BoundaryKind {
    ConditionalJump,
    UnconditionalJump,
    Helper,
    LocalCall,
    KernelCall,
    UnsupportedCall,
    Exit,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Boundary {
    pub id: String,
    pub kind: BoundaryKind,
    pub instruction_ids: Vec<usize>,
    pub targets: Vec<usize>,
    pub fallthrough: Option<usize>,
    pub unresolved: Option<String>,
}

impl InstructionTable {
    pub(crate) fn json(&self) -> Result<Vec<u8>> {
        let mut bytes = serde_json::to_vec_pretty(self)?;
        bytes.push(b'\n');
        Ok(bytes)
    }
}

/// Publish both files together in a fresh directory.
pub fn write_artifacts(directory: &Path, table: &InstructionTable) -> Result<NodeArtifact> {
    let nodes = generate_nodes(table)?;
    let parent = directory
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    fs::create_dir_all(parent)?;
    ensure!(
        !directory.exists(),
        "output directory already exists: {}",
        directory.display()
    );
    let staging = tempfile::tempdir_in(parent)?;
    fs::write(staging.path().join(TABLE_FILE), table.json()?)?;
    let mut nodes_json = serde_json::to_vec_pretty(&nodes)?;
    nodes_json.push(b'\n');
    fs::write(staging.path().join(NODE_FILE), nodes_json)?;
    fs::rename(staging.path(), directory)
        .with_context(|| format!("publish artifacts to {}", directory.display()))?;
    Ok(nodes)
}

/// Read and validate the two typed artifacts. The table digest covers its exact file bytes.
pub fn read_artifacts(directory: &Path) -> Result<(InstructionTable, NodeArtifact)> {
    let table_json = fs::read(directory.join(TABLE_FILE)).context("read instruction table")?;
    let nodes_json = fs::read(directory.join(NODE_FILE)).context("read node artifact")?;
    let table: InstructionTable =
        serde_json::from_slice(&table_json).context("decode instruction table")?;
    let nodes: NodeArtifact =
        serde_json::from_slice(&nodes_json).context("decode node artifact")?;
    ensure!(
        nodes.schema_version == VERSION,
        "unsupported node artifact version"
    );
    let digest = sha256(&table_json);
    ensure!(
        nodes.instruction_table_sha256 == digest,
        "instruction table digest mismatch"
    );
    let mut expected = generate_nodes(&table)?;
    expected.instruction_table_sha256 = digest;
    ensure!(
        nodes == expected,
        "node artifact does not match the instruction table's partition or references"
    );
    Ok((table, nodes))
}
