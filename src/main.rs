use anyhow::{Result, bail};
use sailspan::{FIREWALL, disassemble, read_artifacts, write_artifacts};
use std::path::Path;

fn main() -> Result<()> {
    let args = std::env::args_os().skip(1).collect::<Vec<_>>();
    let output = match args.as_slice() {
        [command, archive, output] if command == "firewall" => {
            let (bytes, source) = FIREWALL.extract(Path::new(archive))?;
            let table = disassemble(&bytes, source)?;
            write_artifacts(Path::new(output), &table)?;
            output
        }
        [command, output] if command == "verify" => output,
        _ => bail!(
            "usage: sailspan firewall <firewall.7z> <new-output-directory> | sailspan verify <output-directory>"
        ),
    };

    let (table, artifact) = read_artifacts(Path::new(output))?;
    println!(
        "{} instructions, {} nodes, {} boundaries",
        table.instructions.len(),
        artifact.nodes.len(),
        artifact.boundaries.len()
    );
    Ok(())
}
