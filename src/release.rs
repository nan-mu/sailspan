use crate::{Source, sha256};
use anyhow::{Context, Result, ensure};
use std::{
    fs,
    io::Write,
    path::Path,
    process::{Command, Output, Stdio},
};

/// Identity and integrity data for a single downloaded release member.
pub struct ReleaseInput<'a> {
    pub tag: &'a str,
    pub asset: &'a str,
    pub archive_sha256: &'a str,
    pub member: &'a str,
    pub member_size: usize,
    pub member_sha256: &'a str,
}

pub const FIREWALL: ReleaseInput<'static> = ReleaseInput {
    tag: "v0.1.2-rc.1",
    asset: "firewall.7z",
    archive_sha256: "88bf2b0345bd14e032fa4a09748bbb0110fc2fd81597a8f76a27db4543d5a37c",
    member: "objects/firewall_bpf.o",
    member_size: 8096,
    member_sha256: "d37ae51e9cf13b9d2ae7f893877dc3b7ada17e03d824a241cc90dba892dedce4",
};

impl ReleaseInput<'_> {
    pub fn extract(&self, archive: &Path) -> Result<(Vec<u8>, Source)> {
        let bytes = fs::read(archive).with_context(|| format!("read {}", archive.display()))?;
        ensure!(
            sha256(&bytes) == self.archive_sha256,
            "{} SHA-256 mismatch",
            self.asset
        );

        // The listing and extraction both use the bytes whose digest was checked.
        let mut checked = tempfile::NamedTempFile::new()?;
        checked.write_all(&bytes)?;
        let listing = self.seven_zip("l", &["-slt", "-ba"], checked.path())?;
        let listing = std::str::from_utf8(&listing.stdout)?;
        let count = listing
            .lines()
            .filter_map(|line| line.strip_prefix("Path = "))
            .filter(|path| *path == self.member)
            .count();
        ensure!(
            count == 1,
            "expected exactly one archive member {}",
            self.member
        );

        let object = self.seven_zip("x", &["-so"], checked.path())?.stdout;
        ensure!(
            object.len() == self.member_size && sha256(&object) == self.member_sha256,
            "{} size or SHA-256 mismatch",
            self.member
        );
        Ok((
            object,
            Source::Release {
                tag: self.tag.into(),
                asset: self.asset.into(),
                member: self.member.into(),
            },
        ))
    }

    fn seven_zip(&self, command: &str, flags: &[&str], archive: &Path) -> Result<Output> {
        let output = Command::new("7z")
            .arg(command)
            .args(flags)
            .args(["-spd", "-t7z", "--"])
            .arg(archive)
            .arg(self.member)
            .stdin(Stdio::null())
            .output()
            .context("run 7z (required for release input)")?;
        ensure!(
            output.status.success(),
            "7z {command} failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
        Ok(output)
    }
}
