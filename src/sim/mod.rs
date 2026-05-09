// SPDX-License-Identifier: GPL-3.0-or-later

use std::collections::BTreeMap;
use std::fmt::{Display, Formatter};
use std::fs;
use std::path::Path;

use crate::backend::pic16::devices::TargetDevice;

const DATA_RAM_BYTES: usize = 512;
const STATUS_ADDR: u16 = 0x03;
const PCL_ADDR: u16 = 0x02;
const FSR_ADDR: u16 = 0x04;
const PCLATH_ADDR: u16 = 0x0A;
const INDF_ADDR: u16 = 0x00;
const STATUS_C_BIT: u8 = 0;
const STATUS_DC_BIT: u8 = 1;
const STATUS_Z_BIT: u8 = 2;
const STATUS_IRP_BIT: u8 = 7;

#[derive(Clone, Debug, Default)]
pub struct ProgramImage {
    words: BTreeMap<u16, u16>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RunOutcome {
    pub stop_pc: u16,
    pub steps: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MachineState {
    pub pc: u16,
    pub w: u8,
    pub status: u8,
    pub pclath: u8,
    pub fsr: u8,
    pub steps: u64,
    pub hardware_stack_top: Option<u16>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TraceRecord {
    pub step: u64,
    pub pc: u16,
    pub word: u16,
    pub instruction: String,
    pub w: u8,
    pub status: u8,
}

#[derive(Debug)]
pub enum SimError {
    InvalidHexRecord(String),
    InvalidHexByte(String),
    MissingInstruction { pc: u16, program_limit: u16 },
    UnsupportedInstruction { pc: u16, word: u16 },
    ReturnStackUnderflow { pc: u16 },
    StepLimitExceeded { pc: u16, max_steps: u64 },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Dest {
    W,
    F,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DecodedInstr {
    Nop,
    Movlw(u8),
    Movwf(u8),
    Movf { f: u8, d: Dest },
    Clrf(u8),
    Clrw,
    Addlw(u8),
    Andlw(u8),
    Iorlw(u8),
    Xorlw(u8),
    Addwf { f: u8, d: Dest },
    Andwf { f: u8, d: Dest },
    Iorwf { f: u8, d: Dest },
    Xorwf { f: u8, d: Dest },
    Subwf { f: u8, d: Dest },
    Rlf { f: u8, d: Dest },
    Rrf { f: u8, d: Dest },
    Swapf { f: u8, d: Dest },
    Bcf { f: u8, b: u8 },
    Bsf { f: u8, b: u8 },
    Btfsc { f: u8, b: u8 },
    Btfss { f: u8, b: u8 },
    Goto(u16),
    Call(u16),
    Retlw(u8),
    Return,
    Retfie,
}

pub struct Pic16Core {
    program: ProgramImage,
    program_limit: u16,
    ram: [u8; DATA_RAM_BYTES],
    pc: u16,
    w: u8,
    hardware_stack: Vec<u16>,
    steps: u64,
}

impl ProgramImage {
    pub fn from_hex_file(path: &Path) -> Result<Self, SimError> {
        let records = fs::read_to_string(path).map_err(|error| {
            SimError::InvalidHexRecord(format!("failed to read `{}`: {error}", path.display()))
        })?;
        Self::from_hex_records(&records)
    }

    pub fn from_hex_records(records: &str) -> Result<Self, SimError> {
        let mut bytes = BTreeMap::new();
        for raw_line in records.lines() {
            let line = raw_line.trim();
            if line.is_empty() {
                continue;
            }
            if !line.starts_with(':') || line.len() < 11 {
                return Err(SimError::InvalidHexRecord(line.to_string()));
            }
            let len = parse_hex_byte(&line[1..3], line)? as usize;
            let addr = parse_hex_u16(&line[3..7], line)?;
            let kind = parse_hex_byte(&line[7..9], line)?;
            if kind != 0 {
                continue;
            }
            if line.len() < 11 + len * 2 {
                return Err(SimError::InvalidHexRecord(line.to_string()));
            }
            for index in 0..len {
                let start = 9 + index * 2;
                let byte = parse_hex_byte(&line[start..start + 2], line)?;
                bytes.insert(addr + index as u16, byte);
            }
        }

        let mut words = BTreeMap::new();
        let mut byte_keys = bytes.keys().copied().collect::<Vec<_>>();
        byte_keys.sort_unstable();
        byte_keys.dedup();
        for byte_addr in byte_keys.into_iter().filter(|addr| addr % 2 == 0) {
            let low = bytes.get(&byte_addr).copied().unwrap_or(0);
            let high = bytes.get(&(byte_addr + 1)).copied().unwrap_or(0) & 0x3F;
            words.insert(byte_addr / 2, u16::from(low) | (u16::from(high) << 8));
        }

        Ok(Self { words })
    }

    pub fn word(&self, pc: u16) -> Option<u16> {
        self.words.get(&pc).copied()
    }

    pub fn is_empty(&self) -> bool {
        self.words.is_empty()
    }
}

impl Pic16Core {
    pub fn new(target: &TargetDevice, program: ProgramImage) -> Self {
        Self {
            program,
            program_limit: target.program_words,
            ram: [0; DATA_RAM_BYTES],
            pc: target.vectors.reset,
            w: 0,
            hardware_stack: Vec::new(),
            steps: 0,
        }
    }

    pub fn with_pc(mut self, pc: u16) -> Self {
        self.pc = pc;
        self
    }

    pub fn pc(&self) -> u16 {
        self.pc
    }

    pub fn w(&self) -> u8 {
        self.w
    }

    pub fn status(&self) -> u8 {
        self.ram[usize::from(STATUS_ADDR)]
    }

    pub fn pclath(&self) -> u8 {
        self.ram[usize::from(PCLATH_ADDR)]
    }

    pub fn fsr(&self) -> u8 {
        self.ram[usize::from(FSR_ADDR)]
    }

    pub fn steps(&self) -> u64 {
        self.steps
    }

    pub fn hardware_stack_top(&self) -> Option<u16> {
        self.hardware_stack.last().copied()
    }

    pub fn state(&self) -> MachineState {
        MachineState {
            pc: self.pc,
            w: self.w,
            status: self.status(),
            pclath: self.pclath(),
            fsr: self.fsr(),
            steps: self.steps,
            hardware_stack_top: self.hardware_stack_top(),
        }
    }

    pub fn read_data(&self, addr: u16) -> u8 {
        let addr = canonical_data_addr(addr);
        self.ram[usize::from(addr)]
    }

    pub fn read_data_u16(&self, addr: u16) -> u16 {
        let lo = self.read_data(addr);
        let hi = self.read_data(addr + 1);
        u16::from(lo) | (u16::from(hi) << 8)
    }

    pub fn read_data_u32(&self, addr: u16) -> u32 {
        let b0 = u32::from(self.read_data(addr));
        let b1 = u32::from(self.read_data(addr + 1));
        let b2 = u32::from(self.read_data(addr + 2));
        let b3 = u32::from(self.read_data(addr + 3));
        b0 | (b1 << 8) | (b2 << 16) | (b3 << 24)
    }

    pub fn run_until_pc(&mut self, stop_pc: u16, max_steps: u64) -> Result<RunOutcome, SimError> {
        while self.pc != stop_pc {
            if self.steps >= max_steps {
                return Err(SimError::StepLimitExceeded {
                    pc: self.pc,
                    max_steps,
                });
            }
            self.step()?;
        }
        Ok(RunOutcome {
            stop_pc,
            steps: self.steps,
        })
    }

    pub fn trace_next(&self) -> Result<TraceRecord, SimError> {
        let pc = self.pc;
        let word = self.program.word(pc).ok_or(SimError::MissingInstruction {
            pc,
            program_limit: self.program_limit,
        })?;
        let instr =
            DecodedInstr::decode(word).ok_or(SimError::UnsupportedInstruction { pc, word })?;
        Ok(TraceRecord {
            step: self.steps,
            pc,
            word,
            instruction: instr.to_string(),
            w: self.w,
            status: self.status(),
        })
    }

    pub fn step_with_trace(&mut self) -> Result<TraceRecord, SimError> {
        let trace = self.trace_next()?;
        self.step()?;
        Ok(trace)
    }

    pub fn step(&mut self) -> Result<(), SimError> {
        let pc = self.pc;
        let word = self.program.word(pc).ok_or(SimError::MissingInstruction {
            pc,
            program_limit: self.program_limit,
        })?;
        let instr =
            DecodedInstr::decode(word).ok_or(SimError::UnsupportedInstruction { pc, word })?;
        let next_pc = pc.wrapping_add(1);
        self.ram[usize::from(PCL_ADDR)] = next_pc as u8;

        self.pc = match instr {
            DecodedInstr::Nop => next_pc,
            DecodedInstr::Movlw(value) => {
                self.w = value;
                next_pc
            }
            DecodedInstr::Movwf(f) => self.write_direct(f, self.w),
            DecodedInstr::Movf { f, d } => {
                let value = self.read_direct(f);
                self.set_status_bit(STATUS_Z_BIT, value == 0);
                match d {
                    Dest::W => {
                        self.w = value;
                        next_pc
                    }
                    Dest::F => self.write_direct(f, value),
                }
            }
            DecodedInstr::Clrf(f) => {
                self.set_status_bit(STATUS_Z_BIT, true);
                self.write_direct(f, 0)
            }
            DecodedInstr::Clrw => {
                self.w = 0;
                self.set_status_bit(STATUS_Z_BIT, true);
                next_pc
            }
            DecodedInstr::Addlw(value) => {
                self.w = self.add8(self.w, value);
                next_pc
            }
            DecodedInstr::Andlw(value) => {
                self.w &= value;
                self.set_status_bit(STATUS_Z_BIT, self.w == 0);
                next_pc
            }
            DecodedInstr::Iorlw(value) => {
                self.w |= value;
                self.set_status_bit(STATUS_Z_BIT, self.w == 0);
                next_pc
            }
            DecodedInstr::Xorlw(value) => {
                self.w ^= value;
                self.set_status_bit(STATUS_Z_BIT, self.w == 0);
                next_pc
            }
            DecodedInstr::Addwf { f, d } => {
                let result = self.add8(self.read_direct(f), self.w);
                match d {
                    Dest::W => {
                        self.w = result;
                        next_pc
                    }
                    Dest::F => self.write_direct(f, result),
                }
            }
            DecodedInstr::Andwf { f, d } => {
                let result = self.read_direct(f) & self.w;
                self.set_status_bit(STATUS_Z_BIT, result == 0);
                match d {
                    Dest::W => {
                        self.w = result;
                        next_pc
                    }
                    Dest::F => self.write_direct(f, result),
                }
            }
            DecodedInstr::Iorwf { f, d } => {
                let result = self.read_direct(f) | self.w;
                self.set_status_bit(STATUS_Z_BIT, result == 0);
                match d {
                    Dest::W => {
                        self.w = result;
                        next_pc
                    }
                    Dest::F => self.write_direct(f, result),
                }
            }
            DecodedInstr::Xorwf { f, d } => {
                let result = self.read_direct(f) ^ self.w;
                self.set_status_bit(STATUS_Z_BIT, result == 0);
                match d {
                    Dest::W => {
                        self.w = result;
                        next_pc
                    }
                    Dest::F => self.write_direct(f, result),
                }
            }
            DecodedInstr::Subwf { f, d } => {
                let result = self.sub8(self.read_direct(f), self.w);
                match d {
                    Dest::W => {
                        self.w = result;
                        next_pc
                    }
                    Dest::F => self.write_direct(f, result),
                }
            }
            DecodedInstr::Rlf { f, d } => {
                let value = self.read_direct(f);
                let carry_in = self.status_bit(STATUS_C_BIT);
                let result = (value << 1) | u8::from(carry_in);
                self.set_status_bit(STATUS_C_BIT, (value & 0x80) != 0);
                match d {
                    Dest::W => {
                        self.w = result;
                        next_pc
                    }
                    Dest::F => self.write_direct(f, result),
                }
            }
            DecodedInstr::Rrf { f, d } => {
                let value = self.read_direct(f);
                let carry_in = if self.status_bit(STATUS_C_BIT) {
                    0x80
                } else {
                    0
                };
                let result = (value >> 1) | carry_in;
                self.set_status_bit(STATUS_C_BIT, (value & 0x01) != 0);
                match d {
                    Dest::W => {
                        self.w = result;
                        next_pc
                    }
                    Dest::F => self.write_direct(f, result),
                }
            }
            DecodedInstr::Swapf { f, d } => {
                let value = self.read_direct(f);
                let result = value.rotate_left(4);
                match d {
                    Dest::W => {
                        self.w = result;
                        next_pc
                    }
                    Dest::F => self.write_direct(f, result),
                }
            }
            DecodedInstr::Bcf { f, b } => {
                let value = self.read_direct(f) & !(1 << b);
                self.write_direct(f, value)
            }
            DecodedInstr::Bsf { f, b } => {
                let value = self.read_direct(f) | (1 << b);
                self.write_direct(f, value)
            }
            DecodedInstr::Btfsc { f, b } => {
                if (self.read_direct(f) & (1 << b)) == 0 {
                    next_pc.wrapping_add(1)
                } else {
                    next_pc
                }
            }
            DecodedInstr::Btfss { f, b } => {
                if (self.read_direct(f) & (1 << b)) != 0 {
                    next_pc.wrapping_add(1)
                } else {
                    next_pc
                }
            }
            DecodedInstr::Goto(target) => self.goto_target(target),
            DecodedInstr::Call(target) => {
                self.hardware_stack.push(next_pc);
                self.goto_target(target)
            }
            DecodedInstr::Retlw(value) => {
                self.w = value;
                self.hardware_stack
                    .pop()
                    .ok_or(SimError::ReturnStackUnderflow { pc })?
            }
            DecodedInstr::Return | DecodedInstr::Retfie => self
                .hardware_stack
                .pop()
                .ok_or(SimError::ReturnStackUnderflow { pc })?,
        };

        self.steps += 1;
        Ok(())
    }

    fn add8(&mut self, lhs: u8, rhs: u8) -> u8 {
        let sum = u16::from(lhs) + u16::from(rhs);
        let result = sum as u8;
        self.set_status_bit(STATUS_C_BIT, sum > 0xFF);
        self.set_status_bit(
            STATUS_DC_BIT,
            u16::from(lhs & 0x0F) + u16::from(rhs & 0x0F) > 0x0F,
        );
        self.set_status_bit(STATUS_Z_BIT, result == 0);
        result
    }

    fn sub8(&mut self, lhs: u8, rhs: u8) -> u8 {
        let result = lhs.wrapping_sub(rhs);
        self.set_status_bit(STATUS_C_BIT, lhs >= rhs);
        self.set_status_bit(STATUS_DC_BIT, (lhs & 0x0F) >= (rhs & 0x0F));
        self.set_status_bit(STATUS_Z_BIT, result == 0);
        result
    }

    fn goto_target(&self, target: u16) -> u16 {
        let page = (u16::from(self.ram[usize::from(PCLATH_ADDR)] & 0x18)) << 8;
        page | (target & 0x07FF)
    }

    fn pcl_target(&self, low: u8) -> u16 {
        (u16::from(self.ram[usize::from(PCLATH_ADDR)] & 0x1F) << 8) | u16::from(low)
    }

    fn status_bit(&self, bit: u8) -> bool {
        (self.ram[usize::from(STATUS_ADDR)] & (1 << bit)) != 0
    }

    fn set_status_bit(&mut self, bit: u8, set: bool) {
        if set {
            self.ram[usize::from(STATUS_ADDR)] |= 1 << bit;
        } else {
            self.ram[usize::from(STATUS_ADDR)] &= !(1 << bit);
        }
    }

    fn write_direct(&mut self, f: u8, value: u8) -> u16 {
        let raw = self.direct_raw_addr(f);
        self.write_raw(raw, value)
    }

    fn read_direct(&self, f: u8) -> u8 {
        let raw = self.direct_raw_addr(f);
        self.read_raw(raw)
    }

    fn direct_raw_addr(&self, f: u8) -> u16 {
        let bank = (self.ram[usize::from(STATUS_ADDR)] >> 5) & 0x03;
        (u16::from(bank) << 7) | u16::from(f & 0x7F)
    }

    fn indirect_raw_addr(&self) -> u16 {
        let high = if self.status_bit(STATUS_IRP_BIT) {
            0x100
        } else {
            0
        };
        high | u16::from(self.ram[usize::from(FSR_ADDR)])
    }

    fn read_raw(&self, raw: u16) -> u8 {
        let addr = canonical_data_addr(raw);
        if addr == INDF_ADDR {
            let indirect = self.indirect_raw_addr();
            if (indirect & 0xFF) == 0 {
                return 0;
            }
            return self.read_raw(indirect);
        }
        self.ram[usize::from(addr)]
    }

    fn write_raw(&mut self, raw: u16, value: u8) -> u16 {
        let addr = canonical_data_addr(raw);
        if addr == INDF_ADDR {
            let indirect = self.indirect_raw_addr();
            if (indirect & 0xFF) == 0 {
                return self.pc.wrapping_add(1);
            }
            return self.write_raw(indirect, value);
        }

        self.ram[usize::from(addr)] = value;
        if addr == PCL_ADDR {
            self.pcl_target(value)
        } else {
            self.pc.wrapping_add(1)
        }
    }
}

impl DecodedInstr {
    fn decode(word: u16) -> Option<Self> {
        match word & 0x3FFF {
            0x0000 => Some(Self::Nop),
            0x0008 => Some(Self::Return),
            0x0009 => Some(Self::Retfie),
            0x0100 => Some(Self::Clrw),
            _ if (word & 0x3800) == 0x2000 => Some(Self::Call(word & 0x07FF)),
            _ if (word & 0x3800) == 0x2800 => Some(Self::Goto(word & 0x07FF)),
            _ if (word & 0x3F00) == 0x3000 => Some(Self::Movlw(word as u8)),
            _ if (word & 0x3F00) == 0x3400 => Some(Self::Retlw(word as u8)),
            _ if (word & 0x3F00) == 0x3800 => Some(Self::Iorlw(word as u8)),
            _ if (word & 0x3F00) == 0x3900 => Some(Self::Andlw(word as u8)),
            _ if (word & 0x3F00) == 0x3A00 => Some(Self::Xorlw(word as u8)),
            _ if (word & 0x3F00) == 0x3E00 => Some(Self::Addlw(word as u8)),
            _ if (word & 0x3F80) == 0x0080 => Some(Self::Movwf((word & 0x7F) as u8)),
            _ if (word & 0x3F80) == 0x0180 => Some(Self::Clrf((word & 0x7F) as u8)),
            _ if (word & 0x3F00) == 0x0200 => Some(Self::Subwf {
                f: (word & 0x7F) as u8,
                d: decode_dest(word),
            }),
            _ if (word & 0x3F00) == 0x0400 => Some(Self::Iorwf {
                f: (word & 0x7F) as u8,
                d: decode_dest(word),
            }),
            _ if (word & 0x3F00) == 0x0500 => Some(Self::Andwf {
                f: (word & 0x7F) as u8,
                d: decode_dest(word),
            }),
            _ if (word & 0x3F00) == 0x0600 => Some(Self::Xorwf {
                f: (word & 0x7F) as u8,
                d: decode_dest(word),
            }),
            _ if (word & 0x3F00) == 0x0700 => Some(Self::Addwf {
                f: (word & 0x7F) as u8,
                d: decode_dest(word),
            }),
            _ if (word & 0x3F00) == 0x0800 => Some(Self::Movf {
                f: (word & 0x7F) as u8,
                d: decode_dest(word),
            }),
            _ if (word & 0x3F00) == 0x0C00 => Some(Self::Rrf {
                f: (word & 0x7F) as u8,
                d: decode_dest(word),
            }),
            _ if (word & 0x3F00) == 0x0D00 => Some(Self::Rlf {
                f: (word & 0x7F) as u8,
                d: decode_dest(word),
            }),
            _ if (word & 0x3F00) == 0x0E00 => Some(Self::Swapf {
                f: (word & 0x7F) as u8,
                d: decode_dest(word),
            }),
            _ if (word & 0x3C00) == 0x1000 => Some(Self::Bcf {
                f: (word & 0x7F) as u8,
                b: ((word >> 7) & 0x07) as u8,
            }),
            _ if (word & 0x3C00) == 0x1400 => Some(Self::Bsf {
                f: (word & 0x7F) as u8,
                b: ((word >> 7) & 0x07) as u8,
            }),
            _ if (word & 0x3C00) == 0x1800 => Some(Self::Btfsc {
                f: (word & 0x7F) as u8,
                b: ((word >> 7) & 0x07) as u8,
            }),
            _ if (word & 0x3C00) == 0x1C00 => Some(Self::Btfss {
                f: (word & 0x7F) as u8,
                b: ((word >> 7) & 0x07) as u8,
            }),
            _ => None,
        }
    }
}

impl Display for TraceRecord {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "{step:06} PC={pc:04X} WORD={word:04X} {instr:<16} W={w:02X} STATUS={status:02X}",
            step = self.step,
            pc = self.pc,
            word = self.word,
            instr = self.instruction,
            w = self.w,
            status = self.status
        )
    }
}

impl Display for DecodedInstr {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Nop => formatter.write_str("NOP"),
            Self::Movlw(value) => write!(formatter, "MOVLW 0x{value:02X}"),
            Self::Movwf(f) => write!(formatter, "MOVWF 0x{f:02X}"),
            Self::Movf { f, d } => write!(formatter, "MOVF 0x{f:02X},{}", d.suffix()),
            Self::Clrf(f) => write!(formatter, "CLRF 0x{f:02X}"),
            Self::Clrw => formatter.write_str("CLRW"),
            Self::Addlw(value) => write!(formatter, "ADDLW 0x{value:02X}"),
            Self::Andlw(value) => write!(formatter, "ANDLW 0x{value:02X}"),
            Self::Iorlw(value) => write!(formatter, "IORLW 0x{value:02X}"),
            Self::Xorlw(value) => write!(formatter, "XORLW 0x{value:02X}"),
            Self::Addwf { f, d } => write!(formatter, "ADDWF 0x{f:02X},{}", d.suffix()),
            Self::Andwf { f, d } => write!(formatter, "ANDWF 0x{f:02X},{}", d.suffix()),
            Self::Iorwf { f, d } => write!(formatter, "IORWF 0x{f:02X},{}", d.suffix()),
            Self::Xorwf { f, d } => write!(formatter, "XORWF 0x{f:02X},{}", d.suffix()),
            Self::Subwf { f, d } => write!(formatter, "SUBWF 0x{f:02X},{}", d.suffix()),
            Self::Rlf { f, d } => write!(formatter, "RLF 0x{f:02X},{}", d.suffix()),
            Self::Rrf { f, d } => write!(formatter, "RRF 0x{f:02X},{}", d.suffix()),
            Self::Swapf { f, d } => write!(formatter, "SWAPF 0x{f:02X},{}", d.suffix()),
            Self::Bcf { f, b } => write!(formatter, "BCF 0x{f:02X},{b}"),
            Self::Bsf { f, b } => write!(formatter, "BSF 0x{f:02X},{b}"),
            Self::Btfsc { f, b } => write!(formatter, "BTFSC 0x{f:02X},{b}"),
            Self::Btfss { f, b } => write!(formatter, "BTFSS 0x{f:02X},{b}"),
            Self::Goto(target) => write!(formatter, "GOTO 0x{target:04X}"),
            Self::Call(target) => write!(formatter, "CALL 0x{target:04X}"),
            Self::Retlw(value) => write!(formatter, "RETLW 0x{value:02X}"),
            Self::Return => formatter.write_str("RETURN"),
            Self::Retfie => formatter.write_str("RETFIE"),
        }
    }
}

impl Dest {
    const fn suffix(self) -> &'static str {
        match self {
            Self::W => "W",
            Self::F => "F",
        }
    }
}

impl Display for SimError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidHexRecord(line) => {
                write!(formatter, "invalid Intel HEX record `{line}`")
            }
            Self::InvalidHexByte(field) => {
                write!(formatter, "invalid Intel HEX byte `{field}`")
            }
            Self::MissingInstruction { pc, program_limit } => {
                write!(
                    formatter,
                    "no instruction loaded at PC 0x{pc:04X} (target program limit 0x{program_limit:04X})"
                )
            }
            Self::UnsupportedInstruction { pc, word } => {
                write!(
                    formatter,
                    "unsupported PIC16 instruction 0x{word:04X} at PC 0x{pc:04X}"
                )
            }
            Self::ReturnStackUnderflow { pc } => {
                write!(formatter, "return-stack underflow at PC 0x{pc:04X}")
            }
            Self::StepLimitExceeded { pc, max_steps } => {
                write!(
                    formatter,
                    "step limit {max_steps} exceeded before stop condition at PC 0x{pc:04X}"
                )
            }
        }
    }
}

impl std::error::Error for SimError {}

fn canonical_data_addr(raw: u16) -> u16 {
    let addr = raw & 0x01FF;
    let low7 = addr & 0x7F;
    if is_common_addr(low7) || (0x70..=0x7F).contains(&low7) {
        low7
    } else {
        addr
    }
}

fn is_common_addr(low7: u16) -> bool {
    matches!(low7, 0x00 | 0x01 | 0x02 | 0x03 | 0x04 | 0x0A | 0x0B)
}

fn decode_dest(word: u16) -> Dest {
    if (word & 0x0080) != 0 {
        Dest::F
    } else {
        Dest::W
    }
}

fn parse_hex_byte(field: &str, line: &str) -> Result<u8, SimError> {
    u8::from_str_radix(field, 16).map_err(|_| SimError::InvalidHexByte(line.to_string()))
}

fn parse_hex_u16(field: &str, line: &str) -> Result<u16, SimError> {
    u16::from_str_radix(field, 16).map_err(|_| SimError::InvalidHexByte(line.to_string()))
}

#[cfg(test)]
mod tests {
    use super::{DecodedInstr, Pic16Core, ProgramImage, SimError, canonical_data_addr};
    use crate::backend::pic16::devices::DeviceRegistry;

    #[test]
    fn decodes_retlw() {
        assert_eq!(
            DecodedInstr::decode(0x345A),
            Some(DecodedInstr::Retlw(0x5A))
        );
    }

    #[test]
    fn canonicalizes_common_and_shared_addresses() {
        assert_eq!(canonical_data_addr(0x83), 0x03);
        assert_eq!(canonical_data_addr(0x1F4), 0x74);
        assert_eq!(canonical_data_addr(0x85), 0x85);
    }

    #[test]
    fn rejects_invalid_hex_record() {
        let error = ProgramImage::from_hex_records("bad").expect_err("invalid hex must fail");
        assert!(matches!(error, SimError::InvalidHexRecord(_)));
    }

    #[test]
    fn rejects_unknown_instruction_word() {
        assert_eq!(DecodedInstr::decode(0x3FFF), None);
    }

    #[test]
    fn errors_on_unsupported_instruction_at_runtime() {
        let registry = DeviceRegistry::new();
        let device = registry.device("pic16f628a").expect("device");
        let program = ProgramImage {
            words: [(0u16, 0x3FFF)].into_iter().collect(),
        };
        let mut core = Pic16Core::new(device, program);
        let error = core.step().expect_err("unsupported word must fail");
        assert!(matches!(
            error,
            SimError::UnsupportedInstruction {
                pc: 0x0000,
                word: 0x3FFF
            }
        ));
    }

    #[test]
    fn executes_basic_indirect_store_and_load() {
        let registry = DeviceRegistry::new();
        let device = registry.device("pic16f628a").expect("device");
        let program = ProgramImage {
            words: [
                (0u16, 0x3029), // movlw 0x29
                (1, 0x0084),    // movwf FSR
                (2, 0x302A),    // movlw 0x2A
                (3, 0x0080),    // movwf INDF
                (4, 0x0800),    // movf INDF,w
                (5, 0x00A0),    // movwf 0x20
                (6, 0x2806),    // goto 0x0006
            ]
            .into_iter()
            .collect(),
        };
        let mut core = Pic16Core::new(device, program);
        core.run_until_pc(6, 32).expect("run");
        assert_eq!(core.read_data(0x29), 0x2A);
        assert_eq!(core.read_data(0x20), 0x2A);
    }

    #[test]
    fn executes_retfie_like_return_stack_pop() {
        let registry = DeviceRegistry::new();
        let device = registry.device("pic16f628a").expect("device");
        let program = ProgramImage {
            words: [(0x0004u16, 0x0009)].into_iter().collect(),
        };
        let mut core = Pic16Core::new(device, program).with_pc(0x0004);
        core.hardware_stack.push(0x0123);

        core.step().expect("retfie step");

        assert_eq!(core.pc(), 0x0123);
        assert_eq!(core.steps(), 1);
        assert!(core.hardware_stack.is_empty());
    }
}
