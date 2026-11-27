use crate::Flow;
use anyhow::{Context, Result, bail, ensure};

/// Decode the instruction families used by the release and the structural fixtures.
pub(crate) fn decode(bytes: &[u8]) -> Result<(u8, String, Option<Flow>)> {
    let first = bytes.get(..8).context("truncated instruction")?;
    let opcode = first[0];
    let dst = first[1] & 0xf;
    let src = first[1] >> 4;
    let offset = i16::from_le_bytes(first[2..4].try_into()?);
    let immediate = i32::from_le_bytes(first[4..8].try_into()?);
    let class = opcode & 7;
    let operation = opcode & 0xf0;
    let register_operand = opcode & 8 != 0;

    if opcode == 0x18 {
        let second = bytes.get(8..16).context("truncated LDDW continuation")?;
        ensure!(second[..4] == [0; 4], "invalid LDDW continuation");
        let value = i64::from_le_bytes([
            first[4], first[5], first[6], first[7], second[4], second[5], second[6], second[7],
        ]);
        let pseudo = if src == 0 {
            String::new()
        } else {
            format!(" (pseudo source {src})")
        };
        return Ok((2, format!("r{dst} = {value} ll{pseudo}"), None));
    }

    let reg = if class == 4 || class == 6 { 'w' } else { 'r' };
    let operand = if register_operand {
        format!("{reg}{src}")
    } else {
        immediate.to_string()
    };
    let (assembly, flow) = match (class, operation) {
        (5, 0x00) if opcode == 0x05 => (
            format!("goto {offset:+}"),
            Some(Flow::Jump {
                offset: i32::from(offset),
                conditional: false,
            }),
        ),
        (6, 0x00) if opcode == 0x06 => (
            format!("goto32 {immediate:+}"),
            Some(Flow::Jump {
                offset: immediate,
                conditional: false,
            }),
        ),
        (5, 0x80) if opcode == 0x85 => (
            match src {
                0 => format!("call {immediate}"),
                _ => format!("call {immediate} (source {src})"),
            },
            Some(Flow::Call {
                mode: src,
                immediate,
            }),
        ),
        (5, 0x80) if opcode == 0x8d => (format!("callx r{immediate}"), Some(Flow::IndirectCall)),
        (5, 0x90) if opcode == 0x95 => ("exit".into(), Some(Flow::Exit)),
        (5 | 6, _) => {
            let condition = match operation {
                0x10 => "==",
                0x20 => ">",
                0x30 => ">=",
                0x40 => "&",
                0x50 => "!=",
                0x60 => "s>",
                0x70 => "s>=",
                0xa0 => "<",
                0xb0 => "<=",
                0xc0 => "s<",
                0xd0 => "s<=",
                _ => bail!("unsupported jump opcode 0x{opcode:02x}"),
            };
            (
                format!("if {reg}{dst} {condition} {operand} goto {offset:+}"),
                Some(Flow::Jump {
                    offset: i32::from(offset),
                    conditional: true,
                }),
            )
        }
        (4 | 7, _) => {
            let expression = match operation {
                0x00 => format!("{reg}{dst} += {operand}"),
                0x10 => format!("{reg}{dst} -= {operand}"),
                0x20 => format!("{reg}{dst} *= {operand}"),
                0x30 if offset == 0 => format!("{reg}{dst} /= {operand}"),
                0x40 => format!("{reg}{dst} |= {operand}"),
                0x50 => format!("{reg}{dst} &= {operand}"),
                0x60 => format!("{reg}{dst} <<= {operand}"),
                0x70 => format!("{reg}{dst} >>= {operand}"),
                0x80 => format!("{reg}{dst} = -{reg}{dst}"),
                0x90 if offset == 0 => format!("{reg}{dst} %= {operand}"),
                0xa0 => format!("{reg}{dst} ^= {operand}"),
                0xb0 if offset == 0 => format!("{reg}{dst} = {operand}"),
                0xc0 => format!("{reg}{dst} s>>= {operand}"),
                _ => bail!("unsupported arithmetic encoding 0x{opcode:02x}, offset {offset}"),
            };
            (expression, None)
        }
        (1..=3, _) if opcode & 0xe0 == 0x60 => {
            let width = match opcode & 0x18 {
                0x00 => 32,
                0x08 => 16,
                0x10 => 8,
                _ => 64,
            };
            let reg = if width == 64 { 'r' } else { 'w' };
            let expression = match class {
                1 => format!("{reg}{dst} = *(u{width} *)(r{src} {offset:+})"),
                2 => format!("*(u{width} *)(r{dst} {offset:+}) = {immediate}"),
                _ => format!("*(u{width} *)(r{dst} {offset:+}) = {reg}{src}"),
            };
            (expression, None)
        }
        _ => bail!("unsupported eBPF opcode 0x{opcode:02x}"),
    };
    Ok((1, assembly, flow))
}
