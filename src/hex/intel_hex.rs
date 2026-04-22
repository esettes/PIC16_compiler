use std::collections::BTreeMap;

use crate::backend::pic16::devices::TargetDevice;

pub struct IntelHexWriter<'a> {
    target: &'a TargetDevice,
}

impl<'a> IntelHexWriter<'a> {
    pub fn new(target: &'a TargetDevice) -> Self {
        Self { target }
    }

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
