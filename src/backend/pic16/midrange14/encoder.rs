use std::collections::BTreeMap;

use crate::diagnostics::DiagnosticBag;

use super::asm::{AsmInstr, AsmLine, AsmProgram, Dest};

pub struct EncoderOutput {
    pub words: BTreeMap<u16, u16>,
    pub labels: BTreeMap<String, u16>,
}

/// Resolves labels and encodes the assembly program into 14-bit PIC16 words.
pub fn encode_program(program: &AsmProgram, diagnostics: &mut DiagnosticBag) -> Option<EncoderOutput> {
    let labels = collect_labels(program, diagnostics)?;
    let mut words = BTreeMap::new();
    let mut pc = 0u16;

    for line in &program.lines {
        match line {
            AsmLine::Org(addr) => pc = *addr,
            AsmLine::Label(_) | AsmLine::Comment(_) => {}
            AsmLine::Instr(instr) => match instr {
                AsmInstr::SetPage(label) => {
                    let Some(addr) = labels.get(label).copied() else {
                        diagnostics.error(
                            "assembler",
                            None,
                            format!("undefined label `{label}`"),
                            None,
                        );
                        return None;
                    };
                    let page = ((addr >> 11) & 0x03) as u8;
                    insert_word(&mut words, pc, encode_instr(&AsmInstr::Bcf { f: 0x0A, b: 3 }));
                    insert_word(&mut words, pc + 1, encode_instr(&AsmInstr::Bcf { f: 0x0A, b: 4 }));
                    insert_word(
                        &mut words,
                        pc + 2,
                        if (page & 0x01) != 0 {
                            encode_instr(&AsmInstr::Bsf { f: 0x0A, b: 3 })
                        } else {
                            encode_instr(&AsmInstr::Nop)
                        },
                    );
                    insert_word(
                        &mut words,
                        pc + 3,
                        if (page & 0x02) != 0 {
                            encode_instr(&AsmInstr::Bsf { f: 0x0A, b: 4 })
                        } else {
                            encode_instr(&AsmInstr::Nop)
                        },
                    );
                    pc += 4;
                }
                AsmInstr::Goto(label) => {
                    let Some(addr) = labels.get(label).copied() else {
                        diagnostics.error(
                            "assembler",
                            None,
                            format!("undefined label `{label}`"),
                            None,
                        );
                        return None;
                    };
                    insert_word(&mut words, pc, 0x2800 | (addr & 0x07FF));
                    pc += 1;
                }
                AsmInstr::Call(label) => {
                    let Some(addr) = labels.get(label).copied() else {
                        diagnostics.error(
                            "assembler",
                            None,
                            format!("undefined label `{label}`"),
                            None,
                        );
                        return None;
                    };
                    insert_word(&mut words, pc, 0x2000 | (addr & 0x07FF));
                    pc += 1;
                }
                _ => {
                    insert_word(&mut words, pc, encode_instr(instr));
                    pc += 1;
                }
            },
        }
    }

    Some(EncoderOutput { words, labels })
}

/// Collects final program-counter addresses for every declared assembly label.
fn collect_labels(
    program: &AsmProgram,
    diagnostics: &mut DiagnosticBag,
) -> Option<BTreeMap<String, u16>> {
    let mut labels = BTreeMap::new();
    let mut pc = 0u16;
    for line in &program.lines {
        match line {
            AsmLine::Org(addr) => pc = *addr,
            AsmLine::Label(label) => {
                if labels.insert(label.clone(), pc).is_some() {
                    diagnostics.error(
                        "assembler",
                        None,
                        format!("duplicate label `{label}`"),
                        None,
                    );
                    return None;
                }
            }
            AsmLine::Instr(instr) => pc += instr.word_len(),
            AsmLine::Comment(_) => {}
        }
    }
    Some(labels)
}

/// Encodes one concrete PIC16 instruction into its 14-bit machine-word form.
fn encode_instr(instr: &AsmInstr) -> u16 {
    match instr {
        AsmInstr::Nop => 0x0000,
        AsmInstr::Movlw(value) => 0x3000 | u16::from(*value),
        AsmInstr::Movwf(f) => 0x0080 | u16::from(*f & 0x7F),
        AsmInstr::Movf { f, d } => 0x0800 | dest_bit(*d) | u16::from(*f & 0x7F),
        AsmInstr::Clrf(f) => 0x0180 | u16::from(*f & 0x7F),
        AsmInstr::Clrw => 0x0100,
        AsmInstr::Addlw(value) => 0x3E00 | u16::from(*value),
        AsmInstr::Andlw(value) => 0x3900 | u16::from(*value),
        AsmInstr::Iorlw(value) => 0x3800 | u16::from(*value),
        AsmInstr::Xorlw(value) => 0x3A00 | u16::from(*value),
        AsmInstr::Addwf { f, d } => 0x0700 | dest_bit(*d) | u16::from(*f & 0x7F),
        AsmInstr::Andwf { f, d } => 0x0500 | dest_bit(*d) | u16::from(*f & 0x7F),
        AsmInstr::Iorwf { f, d } => 0x0400 | dest_bit(*d) | u16::from(*f & 0x7F),
        AsmInstr::Xorwf { f, d } => 0x0600 | dest_bit(*d) | u16::from(*f & 0x7F),
        AsmInstr::Subwf { f, d } => 0x0200 | dest_bit(*d) | u16::from(*f & 0x7F),
        AsmInstr::Bcf { f, b } => 0x1000 | (u16::from(*b & 0x07) << 7) | u16::from(*f & 0x7F),
        AsmInstr::Bsf { f, b } => 0x1400 | (u16::from(*b & 0x07) << 7) | u16::from(*f & 0x7F),
        AsmInstr::Btfsc { f, b } => 0x1800 | (u16::from(*b & 0x07) << 7) | u16::from(*f & 0x7F),
        AsmInstr::Btfss { f, b } => 0x1C00 | (u16::from(*b & 0x07) << 7) | u16::from(*f & 0x7F),
        AsmInstr::Return => 0x0008,
        AsmInstr::Retfie => 0x0009,
        AsmInstr::Goto(_) | AsmInstr::Call(_) | AsmInstr::SetPage(_) => unreachable!("resolved elsewhere"),
    }
}

/// Stores one encoded instruction word while masking it to the 14-bit width.
fn insert_word(words: &mut BTreeMap<u16, u16>, addr: u16, word: u16) {
    words.insert(addr, word & 0x3FFF);
}

/// Converts an instruction destination selector into the encoded destination bit.
const fn dest_bit(dest: Dest) -> u16 {
    match dest {
        Dest::W => 0x0000,
        Dest::F => 0x0080,
    }
}
