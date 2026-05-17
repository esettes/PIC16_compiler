// SPDX-License-Identifier: GPL-3.0-or-later

use std::collections::BTreeMap;

use crate::diagnostics::DiagnosticBag;

use super::asm::{AsmInstr, AsmLine, AsmProgram, Dest};

pub struct EncoderOutput {
    pub words: BTreeMap<u16, u16>,
    pub labels: BTreeMap<String, u16>,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct LinkerRelaxationStats {
    pub passes: usize,
    pub removed_redundant_setpages: usize,
    pub relaxed_same_page_transitions: usize,
}

/// Removes provably redundant page setup pseudo-ops before final encoding.
pub fn relax_page_setup(
    program: &mut AsmProgram,
    diagnostics: &mut DiagnosticBag,
) -> LinkerRelaxationStats {
    let mut total = LinkerRelaxationStats::default();
    for pass in 0..8 {
        let Some(labels) = collect_labels(program, diagnostics) else {
            return total;
        };
        total.passes = pass + 1;
        let mut changed = false;
        let mut index = 0usize;
        while index < program.lines.len() {
            let Some(is_redundant) = setpage_is_redundant_at(program, &labels, index, diagnostics)
            else {
                return total;
            };
            if !is_redundant {
                index += 1;
                continue;
            }

            let mut candidate = program.clone();
            candidate.lines.remove(index);
            let mut candidate_diagnostics = DiagnosticBag::default();
            let candidate_safe = collect_labels(&candidate, &mut candidate_diagnostics)
                .is_some_and(|candidate_labels| {
                    validate_page_safety(&candidate, &candidate_labels, &mut candidate_diagnostics)
                });
            if candidate_safe {
                *program = candidate;
                total.removed_redundant_setpages += 1;
                total.relaxed_same_page_transitions += 1;
                changed = true;
                continue;
            }

            index += 1;
        }
        if diagnostics.has_errors() || !changed {
            break;
        }
    }
    total
}

/// Resolves labels and encodes the assembly program into 14-bit PIC16 words.
pub fn encode_program(
    program: &AsmProgram,
    diagnostics: &mut DiagnosticBag,
) -> Option<EncoderOutput> {
    let labels = collect_labels(program, diagnostics)?;
    if !validate_page_safety(program, &labels, diagnostics) {
        return None;
    }
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
                    insert_word(
                        &mut words,
                        pc,
                        encode_instr(&AsmInstr::Bcf { f: 0x0A, b: 3 }),
                    );
                    insert_word(
                        &mut words,
                        pc + 1,
                        encode_instr(&AsmInstr::Bcf { f: 0x0A, b: 4 }),
                    );
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
                AsmInstr::SetPclPage(label) => {
                    let Some(addr) = labels.get(label).copied() else {
                        diagnostics.error(
                            "assembler",
                            None,
                            format!("undefined label `{label}`"),
                            None,
                        );
                        return None;
                    };
                    let page = ((addr >> 8) & 0x1F) as u8;
                    for bit in 0..=4u8 {
                        insert_word(
                            &mut words,
                            pc + u16::from(bit),
                            encode_instr(&AsmInstr::Bcf { f: 0x0A, b: bit }),
                        );
                    }
                    for bit in 0..=4u8 {
                        insert_word(
                            &mut words,
                            pc + 5 + u16::from(bit),
                            if (page & (1 << bit)) != 0 {
                                encode_instr(&AsmInstr::Bsf { f: 0x0A, b: bit })
                            } else {
                                encode_instr(&AsmInstr::Nop)
                            },
                        );
                    }
                    pc += 10;
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

fn setpage_is_redundant_at(
    program: &AsmProgram,
    labels: &BTreeMap<String, u16>,
    index: usize,
    diagnostics: &mut DiagnosticBag,
) -> Option<bool> {
    let Some(AsmLine::Instr(AsmInstr::SetPage(label))) = program.lines.get(index) else {
        return Some(false);
    };
    let Some(addr) = labels.get(label).copied() else {
        diagnostics.error(
            "assembler",
            None,
            format!("undefined label `{label}`"),
            None,
        );
        return None;
    };
    Some(page_before_line(program, labels, index, diagnostics)? == Some(control_page(addr)))
}

fn page_before_line(
    program: &AsmProgram,
    labels: &BTreeMap<String, u16>,
    index: usize,
    diagnostics: &mut DiagnosticBag,
) -> Option<Option<u8>> {
    let mut pc = 0u16;
    let mut current_page = Some(control_page(pc));

    for line in program.lines.iter().take(index) {
        match line {
            AsmLine::Org(addr) => {
                pc = *addr;
                current_page = Some(control_page(pc));
            }
            AsmLine::Label(label) => {
                let _ = label;
                current_page = Some(control_page(pc));
            }
            AsmLine::Comment(_) => {}
            AsmLine::Instr(AsmInstr::SetPage(label) | AsmInstr::SetPclPage(label)) => {
                let Some(addr) = labels.get(label).copied() else {
                    diagnostics.error(
                        "assembler",
                        None,
                        format!("undefined label `{label}`"),
                        None,
                    );
                    return None;
                };
                current_page = Some(control_page(addr));
                pc += line_word_len(line);
            }
            AsmLine::Instr(instr) => {
                if let AsmInstr::Call(label) = instr
                    && let Some(addr) = labels.get(label).copied()
                {
                    current_page = Some(control_page(addr));
                }
                pc += instr.word_len();
            }
        }
    }

    Some(current_page)
}

fn line_word_len(line: &AsmLine) -> u16 {
    match line {
        AsmLine::Instr(instr) => instr.word_len(),
        _ => 0,
    }
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

/// Validates that every encoded `goto` / `call` has PCLATH set for its target page.
fn validate_page_safety(
    program: &AsmProgram,
    labels: &BTreeMap<String, u16>,
    diagnostics: &mut DiagnosticBag,
) -> bool {
    let mut pc = 0u16;
    let mut current_page = Some(control_page(pc));
    let mut current_symbol = "<start>".to_string();
    let start_errors = diagnostics.diagnostics.len();

    for line in &program.lines {
        match line {
            AsmLine::Org(addr) => {
                pc = *addr;
                current_page = Some(control_page(pc));
                current_symbol = format!("org_0x{pc:04X}");
            }
            AsmLine::Label(label) => {
                current_symbol = label.clone();
                current_page = Some(control_page(pc));
            }
            AsmLine::Comment(_) => {}
            AsmLine::Instr(instr) => {
                match instr {
                    AsmInstr::SetPage(label) | AsmInstr::SetPclPage(label) => {
                        let Some(addr) = labels.get(label).copied() else {
                            diagnostics.error(
                                "assembler",
                                None,
                                format!("undefined label `{label}`"),
                                None,
                            );
                            return false;
                        };
                        current_page = Some(control_page(addr));
                    }
                    AsmInstr::Goto(label) | AsmInstr::Call(label) => {
                        let Some(addr) = labels.get(label).copied() else {
                            diagnostics.error(
                                "assembler",
                                None,
                                format!("undefined label `{label}`"),
                                None,
                            );
                            return false;
                        };
                        let target_page = control_page(addr);
                        if current_page != Some(target_page) {
                            let edge = if matches!(instr, AsmInstr::Call(_)) {
                                "call"
                            } else {
                                "goto"
                            };
                            let from_page = control_page(pc);
                            let known_page = current_page
                                .map(|page| page.to_string())
                                .unwrap_or_else(|| "unknown".to_string());
                            diagnostics.error(
                                "backend",
                                None,
                                format!("unsafe cross-page {edge} in `{current_symbol}`"),
                                Some(format!(
                                    "from: 0x{pc:04X} page {from_page}; target `{label}`: 0x{addr:04X} page {target_page}; PCLATH page before edge: {known_page}"
                                )),
                            );
                        }
                        if matches!(instr, AsmInstr::Call(_)) {
                            current_page = Some(target_page);
                        }
                    }
                    _ => {}
                }
                pc += instr.word_len();
            }
        }
    }

    diagnostics.diagnostics.len() == start_errors
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
        AsmInstr::Rlf { f, d } => 0x0D00 | dest_bit(*d) | u16::from(*f & 0x7F),
        AsmInstr::Rrf { f, d } => 0x0C00 | dest_bit(*d) | u16::from(*f & 0x7F),
        AsmInstr::Swapf { f, d } => 0x0E00 | dest_bit(*d) | u16::from(*f & 0x7F),
        AsmInstr::Bcf { f, b } => 0x1000 | (u16::from(*b & 0x07) << 7) | u16::from(*f & 0x7F),
        AsmInstr::Bsf { f, b } => 0x1400 | (u16::from(*b & 0x07) << 7) | u16::from(*f & 0x7F),
        AsmInstr::Btfsc { f, b } => 0x1800 | (u16::from(*b & 0x07) << 7) | u16::from(*f & 0x7F),
        AsmInstr::Btfss { f, b } => 0x1C00 | (u16::from(*b & 0x07) << 7) | u16::from(*f & 0x7F),
        AsmInstr::Retlw(value) => 0x3400 | u16::from(*value),
        AsmInstr::Return => 0x0008,
        AsmInstr::Retfie => 0x0009,
        AsmInstr::Goto(_) | AsmInstr::Call(_) | AsmInstr::SetPage(_) | AsmInstr::SetPclPage(_) => {
            unreachable!("resolved elsewhere")
        }
    }
}

/// Returns the two-bit page used by PIC16 `goto` / `call` through PCLATH<4:3>.
const fn control_page(addr: u16) -> u8 {
    ((addr >> 11) & 0x03) as u8
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

#[cfg(test)]
mod tests {
    use super::{encode_instr, encode_program, relax_page_setup};
    use crate::backend::pic16::midrange14::asm::{AsmInstr, AsmLine, AsmProgram, Dest};
    use crate::diagnostics::DiagnosticBag;

    #[test]
    /// Verifies the interrupt return instruction encodes to the canonical PIC16 word.
    fn encodes_retfie() {
        assert_eq!(encode_instr(&AsmInstr::Retfie), 0x0009);
    }

    #[test]
    /// Verifies `swapf` uses the expected file-register opcode family.
    fn encodes_swapf() {
        assert_eq!(
            encode_instr(&AsmInstr::Swapf {
                f: 0x70,
                d: Dest::W
            }),
            0x0E70
        );
    }

    #[test]
    /// Verifies `retlw` keeps the literal in the low byte of the 14-bit word.
    fn encodes_retlw() {
        assert_eq!(encode_instr(&AsmInstr::Retlw(0x5A)), 0x345A);
    }

    #[test]
    /// Verifies layout validation rejects raw cross-page gotos without PCLATH setup.
    fn rejects_unsafe_cross_page_goto() {
        let program = AsmProgram {
            lines: vec![
                AsmLine::Org(0x0000),
                AsmLine::Label("from".to_string()),
                AsmLine::Instr(AsmInstr::Goto("target".to_string())),
                AsmLine::Org(0x0800),
                AsmLine::Label("target".to_string()),
                AsmLine::Instr(AsmInstr::Nop),
            ],
        };
        let mut diagnostics = DiagnosticBag::default();

        assert!(encode_program(&program, &mut diagnostics).is_none());
        assert!(
            diagnostics
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("unsafe cross-page goto"))
        );
    }

    #[test]
    /// Verifies SetPage marks a cross-page goto as page-safe.
    fn accepts_page_safe_cross_page_goto() {
        let program = AsmProgram {
            lines: vec![
                AsmLine::Org(0x0000),
                AsmLine::Label("from".to_string()),
                AsmLine::Instr(AsmInstr::SetPage("target".to_string())),
                AsmLine::Instr(AsmInstr::Goto("target".to_string())),
                AsmLine::Org(0x0800),
                AsmLine::Label("target".to_string()),
                AsmLine::Instr(AsmInstr::Nop),
            ],
        };
        let mut diagnostics = DiagnosticBag::default();

        assert!(encode_program(&program, &mut diagnostics).is_some());
        assert!(!diagnostics.has_errors());
    }

    #[test]
    /// Verifies layout validation rejects raw cross-page calls without PCLATH setup.
    fn rejects_unsafe_cross_page_call() {
        let program = AsmProgram {
            lines: vec![
                AsmLine::Org(0x0000),
                AsmLine::Label("from".to_string()),
                AsmLine::Instr(AsmInstr::Call("target".to_string())),
                AsmLine::Org(0x0800),
                AsmLine::Label("target".to_string()),
                AsmLine::Instr(AsmInstr::Return),
            ],
        };
        let mut diagnostics = DiagnosticBag::default();

        assert!(encode_program(&program, &mut diagnostics).is_none());
        assert!(
            diagnostics
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("unsafe cross-page call"))
        );
    }

    #[test]
    /// Verifies linker relaxation removes same-page page setup before a goto.
    fn relaxes_same_page_goto_setpage() {
        let mut program = AsmProgram {
            lines: vec![
                AsmLine::Org(0x0000),
                AsmLine::Label("from".to_string()),
                AsmLine::Instr(AsmInstr::SetPage("target".to_string())),
                AsmLine::Instr(AsmInstr::Goto("target".to_string())),
                AsmLine::Label("target".to_string()),
                AsmLine::Instr(AsmInstr::Nop),
            ],
        };
        let mut diagnostics = DiagnosticBag::default();

        let stats = relax_page_setup(&mut program, &mut diagnostics);

        assert_eq!(stats.removed_redundant_setpages, 1);
        assert!(
            !program
                .lines
                .iter()
                .any(|line| matches!(line, AsmLine::Instr(AsmInstr::SetPage(_))))
        );
        assert!(encode_program(&program, &mut diagnostics).is_some());
        assert!(!diagnostics.has_errors());
    }

    #[test]
    /// Verifies linker relaxation keeps cross-page page setup before a call.
    fn keeps_cross_page_call_setpage() {
        let mut program = AsmProgram {
            lines: vec![
                AsmLine::Org(0x0000),
                AsmLine::Label("from".to_string()),
                AsmLine::Instr(AsmInstr::SetPage("target".to_string())),
                AsmLine::Instr(AsmInstr::Call("target".to_string())),
                AsmLine::Org(0x0800),
                AsmLine::Label("target".to_string()),
                AsmLine::Instr(AsmInstr::Return),
            ],
        };
        let mut diagnostics = DiagnosticBag::default();

        let stats = relax_page_setup(&mut program, &mut diagnostics);

        assert_eq!(stats.removed_redundant_setpages, 0);
        assert!(
            program
                .lines
                .iter()
                .any(|line| matches!(line, AsmLine::Instr(AsmInstr::SetPage(_))))
        );
        assert!(encode_program(&program, &mut diagnostics).is_some());
        assert!(!diagnostics.has_errors());
    }
}
// SPDX-License-Identifier: GPL-3.0-or-later
