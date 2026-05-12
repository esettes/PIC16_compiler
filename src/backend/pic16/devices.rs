// SPDX-License-Identifier: GPL-3.0-or-later

use std::collections::BTreeMap;

use crate::common::source::{ConfigDirective, ConfigDirectiveKind};
use crate::diagnostics::DiagnosticBag;

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

#[derive(Clone, Copy, Debug)]
pub struct ConfigValue {
    pub name: &'static str,
    pub bits: u16,
}

#[derive(Clone, Copy, Debug)]
pub struct ConfigField {
    pub name: &'static str,
    pub mask: u16,
    pub values: &'static [ConfigValue],
}

#[derive(Clone, Copy, Debug)]
pub struct ConfigDescriptor {
    pub address: u16,
    pub valid_mask: u16,
    pub default_word: u16,
    pub reserved_policy: &'static str,
    pub fields: &'static [ConfigField],
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
    pub config: &'static ConfigDescriptor,
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

    /// Resolves `#pragma config` and `__config(...)` directives into one config word.
    pub fn resolve_config_word(
        &self,
        directives: &[ConfigDirective],
        diagnostics: &mut DiagnosticBag,
    ) -> Option<u16> {
        let descriptor = self.config;
        if descriptor.address != self.vectors.config_word
            || descriptor.default_word != self.default_config_word
        {
            diagnostics.error(
                "config",
                None,
                format!("missing target config descriptor for {}", self.name),
                Some("config descriptor must match device vectors/default word".to_string()),
            );
            return None;
        }

        let mut word = descriptor.default_word;
        let mut seen_fields = BTreeMap::<String, String>::new();
        let mut raw_seen = false;
        let mut symbolic_seen = false;

        for directive in directives {
            match &directive.kind {
                ConfigDirectiveKind::Raw(raw) => {
                    if raw_seen || symbolic_seen {
                        diagnostics.error(
                            "config",
                            None,
                            "duplicate config setting",
                            Some("use either one `__config(...)` or symbolic `#pragma config` fields".to_string()),
                        );
                        continue;
                    }
                    raw_seen = true;
                    if raw & !descriptor.valid_mask != 0 {
                        diagnostics.error(
                            "config",
                            None,
                            format!(
                                "raw config word 0x{raw:04X} sets bits outside valid mask 0x{:04X}",
                                descriptor.valid_mask
                            ),
                            Some("clear reserved/unsupported config bits".to_string()),
                        );
                        continue;
                    }
                    word = *raw;
                }
                ConfigDirectiveKind::Field { field, value } => {
                    if raw_seen {
                        diagnostics.error(
                            "config",
                            None,
                            "duplicate config setting",
                            Some("do not mix `__config(...)` with `#pragma config`".to_string()),
                        );
                        continue;
                    }
                    symbolic_seen = true;
                    if let Some(previous) = seen_fields.insert(field.clone(), value.clone()) {
                        diagnostics.error(
                            "config",
                            None,
                            format!(
                                "duplicate config setting `{field}`: `{previous}` then `{value}`"
                            ),
                            None,
                        );
                        continue;
                    }
                    let Some(config_field) =
                        descriptor.fields.iter().find(|item| item.name == field)
                    else {
                        diagnostics.error(
                            "config",
                            None,
                            format!("unknown config field `{field}` for target {}", self.name),
                            Some("check target-specific supported config fields".to_string()),
                        );
                        continue;
                    };
                    let Some(config_value) =
                        config_field.values.iter().find(|item| item.name == value)
                    else {
                        diagnostics.error(
                            "config",
                            None,
                            format!(
                                "unknown config value `{value}` for field `{field}` on target {}",
                                self.name
                            ),
                            Some("check target-specific supported config values".to_string()),
                        );
                        continue;
                    };
                    word = (word & !config_field.mask) | (config_value.bits & config_field.mask);
                }
            }
        }

        if diagnostics.has_errors() {
            None
        } else {
            Some(word)
        }
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

const OFF_ON_LOW_TRUE: [ConfigValue; 2] = [
    ConfigValue {
        name: "ON",
        bits: 0x0000,
    },
    ConfigValue {
        name: "OFF",
        bits: 0xFFFF,
    },
];
const OFF_ON_HIGH_TRUE: [ConfigValue; 2] = [
    ConfigValue {
        name: "OFF",
        bits: 0x0000,
    },
    ConfigValue {
        name: "ON",
        bits: 0xFFFF,
    },
];
const F628A_FOSC_VALUES: [ConfigValue; 8] = [
    ConfigValue {
        name: "LP",
        bits: 0x0000,
    },
    ConfigValue {
        name: "XT",
        bits: 0x0001,
    },
    ConfigValue {
        name: "HS",
        bits: 0x0002,
    },
    ConfigValue {
        name: "EC",
        bits: 0x0003,
    },
    ConfigValue {
        name: "INTRC_NOCLKOUT",
        bits: 0x0004,
    },
    ConfigValue {
        name: "INTRC_CLKOUT",
        bits: 0x0005,
    },
    ConfigValue {
        name: "ER_NOCLKOUT",
        bits: 0x0006,
    },
    ConfigValue {
        name: "ER_CLKOUT",
        bits: 0x0007,
    },
];
const F877A_FOSC_VALUES: [ConfigValue; 4] = [
    ConfigValue {
        name: "LP",
        bits: 0x0000,
    },
    ConfigValue {
        name: "XT",
        bits: 0x0001,
    },
    ConfigValue {
        name: "HS",
        bits: 0x0002,
    },
    ConfigValue {
        name: "RC",
        bits: 0x0003,
    },
];
const F628A_CONFIG_FIELDS: [ConfigField; 8] = [
    ConfigField {
        name: "FOSC",
        mask: 0x0007,
        values: &F628A_FOSC_VALUES,
    },
    ConfigField {
        name: "WDTE",
        mask: 0x0008,
        values: &OFF_ON_HIGH_TRUE,
    },
    ConfigField {
        name: "PWRTE",
        mask: 0x0010,
        values: &OFF_ON_LOW_TRUE,
    },
    ConfigField {
        name: "MCLRE",
        mask: 0x0020,
        values: &OFF_ON_HIGH_TRUE,
    },
    ConfigField {
        name: "BOREN",
        mask: 0x0040,
        values: &OFF_ON_HIGH_TRUE,
    },
    ConfigField {
        name: "LVP",
        mask: 0x0080,
        values: &OFF_ON_HIGH_TRUE,
    },
    ConfigField {
        name: "CPD",
        mask: 0x0100,
        values: &OFF_ON_LOW_TRUE,
    },
    ConfigField {
        name: "CP",
        mask: 0x3E00,
        values: &OFF_ON_LOW_TRUE,
    },
];
const F877A_CONFIG_FIELDS: [ConfigField; 7] = [
    ConfigField {
        name: "FOSC",
        mask: 0x0003,
        values: &F877A_FOSC_VALUES,
    },
    ConfigField {
        name: "WDTE",
        mask: 0x0004,
        values: &OFF_ON_HIGH_TRUE,
    },
    ConfigField {
        name: "PWRTE",
        mask: 0x0008,
        values: &OFF_ON_LOW_TRUE,
    },
    ConfigField {
        name: "BOREN",
        mask: 0x0040,
        values: &OFF_ON_HIGH_TRUE,
    },
    ConfigField {
        name: "LVP",
        mask: 0x0080,
        values: &OFF_ON_HIGH_TRUE,
    },
    ConfigField {
        name: "CPD",
        mask: 0x0100,
        values: &OFF_ON_LOW_TRUE,
    },
    ConfigField {
        name: "CP",
        mask: 0x3000,
        values: &OFF_ON_LOW_TRUE,
    },
];
const F628A_CONFIG: ConfigDescriptor = ConfigDescriptor {
    address: 0x2007,
    valid_mask: 0x3FFF,
    default_word: 0x3F30,
    reserved_policy: "preserve default bits; reject raw bits outside 14-bit config word",
    fields: &F628A_CONFIG_FIELDS,
};
const F877A_CONFIG: ConfigDescriptor = ConfigDescriptor {
    address: 0x2007,
    valid_mask: 0x3FFF,
    default_word: 0x3F32,
    reserved_policy: "preserve default bits; reject raw bits outside 14-bit config word",
    fields: &F877A_CONFIG_FIELDS,
};

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
        config: &F628A_CONFIG,
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
        config: &F877A_CONFIG,
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
        assert_eq!(device.config.address, 0x2007);
        assert_eq!(device.config.default_word, device.default_config_word);
        assert!(
            device
                .config
                .fields
                .iter()
                .any(|field| field.name == "FOSC")
        );
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
        assert_eq!(device.config.address, 0x2007);
        assert_eq!(device.config.default_word, device.default_config_word);
        assert!(
            device
                .config
                .fields
                .iter()
                .any(|field| field.name == "FOSC")
        );
        assert!(!device.allocatable_gpr.is_empty());
        assert!(!device.reserved_ram.is_empty());
        assert!(device.rom_table_region.contains(0x1FFF));
    }
}
// SPDX-License-Identifier: GPL-3.0-or-later
