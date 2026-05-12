// SPDX-License-Identifier: GPL-3.0-or-later

use std::collections::BTreeMap;
use std::fmt::Write;

use crate::backend::pic16::devices::TargetDevice;

#[derive(Clone, Debug)]
pub struct HexValidationReport {
    pub text: String,
}

#[derive(Debug)]
struct HexRecord {
    addr: u16,
    kind: u8,
    data: Vec<u8>,
}

pub struct IntelHexWriter<'a> {
    target: &'a TargetDevice,
}

impl<'a> IntelHexWriter<'a> {
    /// Creates a HEX writer configured for a specific PIC16 device descriptor.
    pub fn new(target: &'a TargetDevice) -> Self {
        Self { target }
    }

    /// Emits Intel HEX records for encoded words plus the device config word.
    pub fn emit(&self, words: &BTreeMap<u16, u16>, config_word: u16) -> String {
        let mut bytes = BTreeMap::new();
        for (addr, word) in words {
            let byte_addr = addr * 2;
            bytes.insert(byte_addr, (word & 0x00FF) as u8);
            bytes.insert(byte_addr + 1, ((word >> 8) & 0x003F) as u8);
        }

        let config_addr = self.target.vectors.config_word * 2;
        bytes.insert(config_addr, (config_word & 0x00FF) as u8);
        bytes.insert(config_addr + 1, ((config_word >> 8) & 0x00FF) as u8);

        let mut output = String::new();
        let entries = bytes.into_iter().collect::<Vec<_>>();
        let mut index = 0usize;
        while index < entries.len() {
            let start_addr = entries[index].0;
            let mut chunk = vec![entries[index].1];
            let mut next = index + 1;
            while next < entries.len()
                && entries[next].0 == start_addr + (next - index) as u16
                && chunk.len() < 16
            {
                chunk.push(entries[next].1);
                next += 1;
            }
            output.push_str(&encode_record(start_addr, &chunk));
            output.push('\n');
            index = next;
        }
        output.push_str(":00000001FF\n");
        output
    }
}

/// Validates final Intel HEX output against target memory and encoded words.
pub fn validate_hex_output(
    target: &TargetDevice,
    words: &BTreeMap<u16, u16>,
    config_word: u16,
    hex: &str,
) -> Result<HexValidationReport, Vec<String>> {
    let records = parse_hex_records(hex)?;
    let mut errors = Vec::new();
    if !matches!(records.last(), Some(record) if record.kind == 0x01) {
        errors.push("Intel HEX EOF record missing".to_string());
    }
    let mut bytes = BTreeMap::<u16, u8>::new();
    for record in &records {
        match record.kind {
            0x00 => {
                for (offset, byte) in record.data.iter().enumerate() {
                    let addr = record.addr.saturating_add(offset as u16);
                    bytes.insert(addr, *byte);
                }
            }
            0x01 => {
                if !record.data.is_empty() {
                    errors.push("Intel HEX EOF record must not contain data".to_string());
                }
            }
            kind => errors.push(format!("unsupported Intel HEX record type 0x{kind:02X}")),
        }
    }

    if !words.contains_key(&target.vectors.reset) {
        errors.push(format!(
            "missing reset vector at 0x{:04X}",
            target.vectors.reset
        ));
    }
    if !words.contains_key(&target.vectors.interrupt) {
        errors.push(format!(
            "missing interrupt vector at 0x{:04X}",
            target.vectors.interrupt
        ));
    }
    if words.contains_key(&target.vectors.config_word) {
        errors.push(format!(
            "program word overlaps config word at 0x{:04X}",
            target.vectors.config_word
        ));
    }

    let mut out_of_range_count = 0usize;
    let mut highest_out_of_range = None::<u16>;
    for (addr, word) in words {
        if !target.program_memory.contains(*addr) {
            out_of_range_count += 1;
            highest_out_of_range =
                Some(highest_out_of_range.map_or(*addr, |highest| highest.max(*addr)));
        }
        if word & !0x3FFF != 0 {
            errors.push(format!(
                "word 0x{word:04X} at 0x{addr:04X} is not a 14-bit PIC16 word"
            ));
        }
        let byte_addr = addr.saturating_mul(2);
        let low = bytes.get(&byte_addr).copied();
        let high = bytes.get(&(byte_addr + 1)).copied();
        if low != Some((word & 0x00FF) as u8) || high != Some(((word >> 8) & 0x003F) as u8) {
            errors.push(format!(
                "HEX bytes do not match word 0x{word:04X} at 0x{addr:04X}"
            ));
        }
    }
    if let Some(highest) = highest_out_of_range {
        errors.push(format!(
            "{out_of_range_count} program word(s) outside target {} range 0x{:04X}..0x{:04X}; highest=0x{highest:04X}",
            target.name, target.program_memory.start, target.program_memory.end
        ));
    }

    let config_addr = target.vectors.config_word.saturating_mul(2);
    if bytes.get(&config_addr).copied() != Some((config_word & 0x00FF) as u8)
        || bytes.get(&(config_addr + 1)).copied() != Some(((config_word >> 8) & 0x00FF) as u8)
    {
        errors.push(format!(
            "config word 0x{config_word:04X} missing at target address 0x{:04X}",
            target.vectors.config_word
        ));
    }

    if errors.is_empty() {
        Ok(HexValidationReport {
            text: render_hex_validation_report(target, words, config_word, records.len()),
        })
    } else {
        Err(errors)
    }
}

fn render_hex_validation_report(
    target: &TargetDevice,
    words: &BTreeMap<u16, u16>,
    config_word: u16,
    record_count: usize,
) -> String {
    let mut output = String::new();
    let _ = writeln!(output, "HEX validation");
    let _ = writeln!(output, "--------------");
    let _ = writeln!(output, "target: {}", target.name);
    let _ = writeln!(
        output,
        "program range: 0x{:04X}..0x{:04X}",
        target.program_memory.start, target.program_memory.end
    );
    let _ = writeln!(output, "program words: {}", words.len());
    let _ = writeln!(
        output,
        "vectors: reset=0x{:04X} interrupt=0x{:04X}",
        target.vectors.reset, target.vectors.interrupt
    );
    let _ = writeln!(
        output,
        "config: 0x{config_word:04X} @ 0x{:04X}",
        target.vectors.config_word
    );
    let _ = writeln!(output, "records: {record_count}");
    let _ = writeln!(output, "checksum: ok");
    let _ = writeln!(output, "eof: ok");
    output
}

fn parse_hex_records(hex: &str) -> Result<Vec<HexRecord>, Vec<String>> {
    let mut records = Vec::new();
    let mut errors = Vec::new();
    for (line_index, raw_line) in hex.lines().enumerate() {
        let line = raw_line.trim();
        if line.is_empty() {
            continue;
        }
        match parse_hex_record(line) {
            Ok(record) => records.push(record),
            Err(message) => errors.push(format!("line {}: {message}", line_index + 1)),
        }
    }
    if records.is_empty() {
        errors.push("Intel HEX has no records".to_string());
    }
    if errors.is_empty() {
        Ok(records)
    } else {
        Err(errors)
    }
}

fn parse_hex_record(line: &str) -> Result<HexRecord, String> {
    if !line.starts_with(':') {
        return Err("record does not start with `:`".to_string());
    }
    if line.len() < 11 || !(line.len() - 1).is_multiple_of(2) {
        return Err("record has invalid length".to_string());
    }
    let bytes = (1..line.len())
        .step_by(2)
        .map(|index| {
            u8::from_str_radix(&line[index..index + 2], 16)
                .map_err(|_| "record contains non-hex byte".to_string())
        })
        .collect::<Result<Vec<_>, _>>()?;
    let len = bytes[0] as usize;
    if bytes.len() != len + 5 {
        return Err("record byte count does not match data length".to_string());
    }
    let checksum = bytes.iter().fold(0u8, |sum, byte| sum.wrapping_add(*byte));
    if checksum != 0 {
        return Err("Intel HEX checksum mismatch".to_string());
    }
    let addr = ((bytes[1] as u16) << 8) | bytes[2] as u16;
    let kind = bytes[3];
    let data = bytes[4..4 + len].to_vec();
    Ok(HexRecord { addr, kind, data })
}

/// Encodes one Intel HEX data record and computes its checksum.
fn encode_record(addr: u16, bytes: &[u8]) -> String {
    let mut checksum: u8 = bytes.len() as u8;
    checksum = checksum.wrapping_add((addr >> 8) as u8);
    checksum = checksum.wrapping_add((addr & 0xFF) as u8);
    let mut data = String::new();
    for byte in bytes {
        checksum = checksum.wrapping_add(*byte);
        data.push_str(&format!("{byte:02X}"));
    }
    let checksum = (!checksum).wrapping_add(1);
    format!(":{:02X}{addr:04X}00{}{checksum:02X}", bytes.len(), data)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::IntelHexWriter;
    use super::validate_hex_output;
    use crate::backend::pic16::devices::DeviceRegistry;

    #[test]
    /// Verifies emitted HEX still contains the config-word record and EOF marker.
    fn emits_config_record_and_eof_for_pic16_targets() {
        let registry = DeviceRegistry::new();
        let target = registry.device("pic16f628a").expect("device");
        let mut words = BTreeMap::new();
        words.insert(0x0000, 0x2805);
        words.insert(0x0004, 0x0009);

        let hex = IntelHexWriter::new(target).emit(&words, target.default_config_word);
        assert!(hex.contains(":02400E00"));
        assert!(hex.ends_with(":00000001FF\n"));
        validate_hex_output(target, &words, target.default_config_word, &hex).expect("valid hex");
    }

    #[test]
    fn rejects_bad_hex_checksum() {
        let registry = DeviceRegistry::new();
        let target = registry.device("pic16f628a").expect("device");
        let mut words = BTreeMap::new();
        words.insert(0x0000, 0x2805);
        words.insert(0x0004, 0x0009);
        let bad = ":02000000052800\n:00000001FF\n";
        let error = validate_hex_output(target, &words, target.default_config_word, bad)
            .expect_err("bad checksum");
        assert!(error.iter().any(|line| line.contains("checksum")));
    }

    #[test]
    fn rejects_missing_hex_eof() {
        let registry = DeviceRegistry::new();
        let target = registry.device("pic16f628a").expect("device");
        let mut words = BTreeMap::new();
        words.insert(0x0000, 0x2805);
        words.insert(0x0004, 0x0009);
        let hex = IntelHexWriter::new(target).emit(&words, target.default_config_word);
        let hex = hex.trim_end_matches(":00000001FF\n");
        let error = validate_hex_output(target, &words, target.default_config_word, hex)
            .expect_err("missing eof");
        assert!(error.iter().any(|line| line.contains("EOF")));
    }

    #[test]
    fn rejects_config_overlap() {
        let registry = DeviceRegistry::new();
        let target = registry.device("pic16f628a").expect("device");
        let mut words = BTreeMap::new();
        words.insert(0x0000, 0x2805);
        words.insert(0x0004, 0x0009);
        words.insert(target.vectors.config_word, 0x3FFF);
        let hex = IntelHexWriter::new(target).emit(&words, target.default_config_word);
        let error = validate_hex_output(target, &words, target.default_config_word, &hex)
            .expect_err("config overlap");
        assert!(
            error
                .iter()
                .any(|line| line.contains("overlaps config word"))
        );
    }
}
// SPDX-License-Identifier: GPL-3.0-or-later
