use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug)]
pub struct MemoryRange {
    pub start: u16,
    pub end: u16,
}

impl MemoryRange {
    pub const fn size(self) -> u16 {
        self.end - self.start + 1
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
    pub allocatable_gpr: &'static [MemoryRange],
    pub sfrs: &'static [DeviceRegister],
    pub default_config_word: u16,
    pub capabilities: &'static [&'static str],
}

impl TargetDevice {
    pub fn sfr_address(&self, name: &str) -> Option<u16> {
        self.sfrs
            .iter()
            .find(|register| register.name == name)
            .map(|register| register.address)
    }

    pub fn sfr_map(&self) -> BTreeMap<String, u16> {
        self.sfrs
            .iter()
            .map(|register| (register.name.to_string(), register.address))
            .collect()
    }
}

pub struct DeviceRegistry {
    devices: Vec<TargetDevice>,
}

impl Default for DeviceRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl DeviceRegistry {
    pub fn new() -> Self {
        Self {
            devices: vec![pic16f628a(), pic16f877a()],
        }
    }

    pub fn device(&self, name: &str) -> Option<&TargetDevice> {
        self.devices
            .iter()
            .find(|device| device.name.eq_ignore_ascii_case(name))
    }

    pub fn devices(&self) -> &[TargetDevice] {
        &self.devices
    }
}

const F628A_GPR: [MemoryRange; 1] = [MemoryRange { start: 0x20, end: 0x6F }];
const F877A_GPR: [MemoryRange; 1] = [MemoryRange { start: 0x20, end: 0x7F }];

const F628A_SFRS: [DeviceRegister; 17] = [
    DeviceRegister { name: "INDF", address: 0x00 },
    DeviceRegister { name: "TMR0", address: 0x01 },
    DeviceRegister { name: "PCL", address: 0x02 },
    DeviceRegister { name: "STATUS", address: 0x03 },
    DeviceRegister { name: "FSR", address: 0x04 },
    DeviceRegister { name: "PORTA", address: 0x05 },
    DeviceRegister { name: "PORTB", address: 0x06 },
    DeviceRegister { name: "PCLATH", address: 0x0A },
    DeviceRegister { name: "INTCON", address: 0x0B },
    DeviceRegister { name: "TMR1L", address: 0x0E },
    DeviceRegister { name: "TMR1H", address: 0x0F },
    DeviceRegister { name: "T1CON", address: 0x10 },
    DeviceRegister { name: "TMR2", address: 0x11 },
    DeviceRegister { name: "T2CON", address: 0x12 },
    DeviceRegister { name: "CCP1CON", address: 0x17 },
    DeviceRegister { name: "TRISA", address: 0x85 },
    DeviceRegister { name: "TRISB", address: 0x86 },
];

const F877A_SFRS: [DeviceRegister; 18] = [
    DeviceRegister { name: "INDF", address: 0x00 },
    DeviceRegister { name: "TMR0", address: 0x01 },
    DeviceRegister { name: "PCL", address: 0x02 },
    DeviceRegister { name: "STATUS", address: 0x03 },
    DeviceRegister { name: "FSR", address: 0x04 },
    DeviceRegister { name: "PORTA", address: 0x05 },
    DeviceRegister { name: "PORTB", address: 0x06 },
    DeviceRegister { name: "PORTC", address: 0x07 },
    DeviceRegister { name: "PORTD", address: 0x08 },
    DeviceRegister { name: "PORTE", address: 0x09 },
    DeviceRegister { name: "PCLATH", address: 0x0A },
    DeviceRegister { name: "INTCON", address: 0x0B },
    DeviceRegister { name: "TRISA", address: 0x85 },
    DeviceRegister { name: "TRISB", address: 0x86 },
    DeviceRegister { name: "TRISC", address: 0x87 },
    DeviceRegister { name: "TRISD", address: 0x88 },
    DeviceRegister { name: "TRISE", address: 0x89 },
    DeviceRegister { name: "ADCON1", address: 0x9F },
];

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
        allocatable_gpr: &F628A_GPR,
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
        allocatable_gpr: &F877A_GPR,
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
