// SPDX-License-Identifier: GPL-3.0-or-later

use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug)]
pub struct MemoryRange {
    pub start: u16,
    pub end: u16,
}

impl MemoryRange {
    /// Returns the inclusive size of a RAM range in bytes.
    pub const fn size(self) -> u16 {
        self.end - self.start + 1
    }

    /// Returns true when an address is inside the inclusive range.
    pub const fn contains(self, address: u16) -> bool {
        address >= self.start && address <= self.end
    }
}

#[derive(Clone, Copy, Debug)]
pub struct DeviceRegister {
    pub name: &'static str,
    pub address: u16,
}

#[derive(Clone, Copy, Debug)]
pub struct DeviceVectors {
    pub reset: u16,
    pub interrupt: u16,
    pub config_word: u16,
}

#[derive(Clone, Debug)]
pub struct TargetDevice {
    pub name: &'static str,
    pub family: &'static str,
    pub description: &'static str,
    pub program_words: u16,
    pub data_ram_bytes: u16,
    pub eeprom_bytes: u16,
    pub bank_count: u8,
    pub vectors: DeviceVectors,
    pub program_memory: MemoryRange,
    pub allocatable_gpr: &'static [MemoryRange],
    pub shared_gpr: &'static [MemoryRange],
    pub reserved_ram: &'static [MemoryRange],
    pub default_stack_region: MemoryRange,
    pub rom_table_region: MemoryRange,
    pub sfrs: &'static [DeviceRegister],
    pub default_config_word: u16,
    pub capabilities: &'static [&'static str],
}

impl TargetDevice {
    /// Looks up the absolute address of a named special-function register.
    pub fn sfr_address(&self, name: &str) -> Option<u16> {
        self.sfrs
            .iter()
            .find(|register| register.name == name)
            .map(|register| register.address)
    }

    /// Builds a name-to-address map for the device SFR table.
    pub fn sfr_map(&self) -> BTreeMap<String, u16> {
        self.sfrs
            .iter()
            .map(|register| (register.name.to_string(), register.address))
            .collect()
    }

    /// Returns bytes in the backend-modeled allocatable data ranges.
    pub fn allocatable_ram_bytes(&self) -> u16 {
        self.allocatable_gpr.iter().map(|range| range.size()).sum()
    }

    /// Returns bytes in allocatable plus shared backend-modeled GPR ranges.
    pub fn modeled_data_ram_bytes(&self) -> u16 {
        self.allocatable_ram_bytes()
            + self
                .shared_gpr
                .iter()
                .map(|range| range.size())
                .sum::<u16>()
    }
}

pub struct DeviceRegistry {
    devices: Vec<TargetDevice>,
}

impl Default for DeviceRegistry {
    /// Creates the default registry containing all built-in target descriptors.
    fn default() -> Self {
        Self::new()
    }
}

impl DeviceRegistry {
    /// Creates a registry with the built-in PIC16 target descriptors.
    pub fn new() -> Self {
        Self {
            devices: vec![pic16f628a(), pic16f877a()],
        }
    }

    /// Finds a device descriptor by case-insensitive target name.
    pub fn device(&self, name: &str) -> Option<&TargetDevice> {
        self.devices
            .iter()
            .find(|device| device.name.eq_ignore_ascii_case(name))
    }

    /// Returns all registered device descriptors.
    pub fn devices(&self) -> &[TargetDevice] {
        &self.devices
    }
}

const F628A_GPR: [MemoryRange; 1] = [MemoryRange {
    start: 0x20,
    end: 0x6F,
}];
const F877A_GPR: [MemoryRange; 1] = [MemoryRange {
    start: 0x20,
    end: 0x6F,
}];
const SHARED_GPR: [MemoryRange; 1] = [MemoryRange {
    start: 0x70,
    end: 0x7F,
}];
const F628A_RESERVED_RAM: [MemoryRange; 4] = [
    MemoryRange {
        start: 0x0000,
        end: 0x001F,
    },
    MemoryRange {
        start: 0x0080,
        end: 0x009F,
    },
    MemoryRange {
        start: 0x0100,
        end: 0x011F,
    },
    MemoryRange {
        start: 0x0180,
        end: 0x019F,
    },
];
const F877A_RESERVED_RAM: [MemoryRange; 4] = [
    MemoryRange {
        start: 0x0000,
        end: 0x001F,
    },
    MemoryRange {
        start: 0x0080,
        end: 0x009F,
    },
    MemoryRange {
        start: 0x0100,
        end: 0x011F,
    },
    MemoryRange {
        start: 0x0180,
        end: 0x019F,
    },
];

const F628A_SFRS: [DeviceRegister; 17] = [
    DeviceRegister {
        name: "INDF",
        address: 0x00,
    },
    DeviceRegister {
        name: "TMR0",
        address: 0x01,
    },
    DeviceRegister {
        name: "PCL",
        address: 0x02,
    },
    DeviceRegister {
        name: "STATUS",
        address: 0x03,
    },
    DeviceRegister {
        name: "FSR",
        address: 0x04,
    },
    DeviceRegister {
        name: "PORTA",
        address: 0x05,
    },
    DeviceRegister {
        name: "PORTB",
        address: 0x06,
    },
    DeviceRegister {
        name: "PCLATH",
        address: 0x0A,
    },
    DeviceRegister {
        name: "INTCON",
        address: 0x0B,
    },
    DeviceRegister {
        name: "TMR1L",
        address: 0x0E,
    },
    DeviceRegister {
        name: "TMR1H",
        address: 0x0F,
    },
    DeviceRegister {
        name: "T1CON",
        address: 0x10,
    },
    DeviceRegister {
        name: "TMR2",
        address: 0x11,
    },
    DeviceRegister {
        name: "T2CON",
        address: 0x12,
    },
    DeviceRegister {
        name: "CCP1CON",
        address: 0x17,
    },
    DeviceRegister {
        name: "TRISA",
        address: 0x85,
    },
    DeviceRegister {
        name: "TRISB",
        address: 0x86,
    },
];

const F877A_SFRS: [DeviceRegister; 18] = [
    DeviceRegister {
        name: "INDF",
        address: 0x00,
    },
    DeviceRegister {
        name: "TMR0",
        address: 0x01,
    },
    DeviceRegister {
        name: "PCL",
        address: 0x02,
    },
    DeviceRegister {
        name: "STATUS",
        address: 0x03,
    },
    DeviceRegister {
        name: "FSR",
        address: 0x04,
    },
    DeviceRegister {
        name: "PORTA",
        address: 0x05,
    },
    DeviceRegister {
        name: "PORTB",
        address: 0x06,
    },
    DeviceRegister {
        name: "PORTC",
        address: 0x07,
    },
    DeviceRegister {
        name: "PORTD",
        address: 0x08,
    },
    DeviceRegister {
        name: "PORTE",
        address: 0x09,
    },
    DeviceRegister {
        name: "PCLATH",
        address: 0x0A,
    },
    DeviceRegister {
        name: "INTCON",
        address: 0x0B,
    },
    DeviceRegister {
        name: "TRISA",
        address: 0x85,
    },
    DeviceRegister {
        name: "TRISB",
        address: 0x86,
    },
    DeviceRegister {
        name: "TRISC",
        address: 0x87,
    },
    DeviceRegister {
        name: "TRISD",
        address: 0x88,
    },
    DeviceRegister {
        name: "TRISE",
        address: 0x89,
    },
    DeviceRegister {
        name: "ADCON1",
        address: 0x9F,
    },
];

/// Builds the descriptor for the PIC16F628A target.
fn pic16f628a() -> TargetDevice {
    TargetDevice {
        name: "pic16f628a",
        family: "midrange14",
        description: "PIC16F628A Flash-based 8-bit MCU",
        program_words: 2048,
        data_ram_bytes: 224,
        eeprom_bytes: 128,
        bank_count: 4,
        vectors: DeviceVectors {
            reset: 0x0000,
            interrupt: 0x0004,
            config_word: 0x2007,
        },
        program_memory: MemoryRange {
            start: 0x0000,
            end: 0x07FF,
        },
        allocatable_gpr: &F628A_GPR,
        shared_gpr: &SHARED_GPR,
        reserved_ram: &F628A_RESERVED_RAM,
        default_stack_region: F628A_GPR[0],
        rom_table_region: MemoryRange {
            start: 0x0005,
            end: 0x07FF,
        },
        sfrs: &F628A_SFRS,
        default_config_word: 0x3F30,
        capabilities: &[
            "harvard",
            "14-bit instructions",
            "banked ram",
            "program paging",
            "single interrupt vector",
            "internal oscillator",
        ],
    }
}

/// Builds the descriptor for the PIC16F877A target.
fn pic16f877a() -> TargetDevice {
    TargetDevice {
        name: "pic16f877a",
        family: "midrange14",
        description: "PIC16F877A Flash-based 8-bit MCU",
        program_words: 8192,
        data_ram_bytes: 368,
        eeprom_bytes: 256,
        bank_count: 4,
        vectors: DeviceVectors {
            reset: 0x0000,
            interrupt: 0x0004,
            config_word: 0x2007,
        },
        program_memory: MemoryRange {
            start: 0x0000,
            end: 0x1FFF,
        },
        allocatable_gpr: &F877A_GPR,
        shared_gpr: &SHARED_GPR,
        reserved_ram: &F877A_RESERVED_RAM,
        default_stack_region: F877A_GPR[0],
        rom_table_region: MemoryRange {
            start: 0x0005,
            end: 0x1FFF,
        },
        sfrs: &F877A_SFRS,
        default_config_word: 0x3F32,
        capabilities: &[
            "harvard",
            "14-bit instructions",
            "banked ram",
            "program paging",
            "single interrupt vector",
            "ports a-e",
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::DeviceRegistry;

    #[test]
    fn pic16f628a_resource_descriptor_is_explicit() {
        let registry = DeviceRegistry::new();
        let device = registry.device("pic16f628a").expect("descriptor");
        assert_eq!(device.program_memory.start, 0x0000);
        assert_eq!(device.program_memory.end, 0x07FF);
        assert_eq!(device.vectors.reset, 0x0000);
        assert_eq!(device.vectors.interrupt, 0x0004);
        assert_eq!(device.vectors.config_word, 0x2007);
        assert!(!device.allocatable_gpr.is_empty());
        assert!(!device.reserved_ram.is_empty());
        assert!(device.rom_table_region.contains(0x07FF));
    }

    #[test]
    fn pic16f877a_resource_descriptor_is_explicit() {
        let registry = DeviceRegistry::new();
        let device = registry.device("pic16f877a").expect("descriptor");
        assert_eq!(device.program_memory.start, 0x0000);
        assert_eq!(device.program_memory.end, 0x1FFF);
        assert_eq!(device.vectors.reset, 0x0000);
        assert_eq!(device.vectors.interrupt, 0x0004);
        assert_eq!(device.vectors.config_word, 0x2007);
        assert!(!device.allocatable_gpr.is_empty());
        assert!(!device.reserved_ram.is_empty());
        assert!(device.rom_table_region.contains(0x1FFF));
    }
}
// SPDX-License-Identifier: GPL-3.0-or-later
