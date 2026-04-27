use std::fmt::Write;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Dest {
    W,
    F,
}

#[derive(Clone, Debug)]
pub struct AsmProgram {
    pub lines: Vec<AsmLine>,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct PeepholeStats {
    pub removed_instructions: usize,
    pub self_moves_removed: usize,
    pub duplicate_writes_removed: usize,
    pub duplicate_bit_ops_removed: usize,
    pub duplicate_setpages_removed: usize,
    pub overwritten_w_loads_removed: usize,
}

#[derive(Clone, Debug)]
pub enum AsmLine {
    Org(u16),
    Label(String),
    Instr(AsmInstr),
    Comment(String),
}

#[derive(Clone, Debug)]
pub enum AsmInstr {
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
    Goto(String),
    Call(String),
    Return,
    Retfie,
    SetPage(String),
}

impl AsmProgram {
    /// Creates an empty assembly program buffer.
    pub fn new() -> Self {
        Self { lines: Vec::new() }
    }

    /// Appends one assembly line without altering label or address state.
    pub fn push(&mut self, line: AsmLine) {
        self.lines.push(line);
    }

    /// Applies conservative backend peephole cleanups that preserve observable behavior.
    pub fn peephole_optimize(&mut self) -> PeepholeStats {
        let mut stats = PeepholeStats::default();
        let mut optimized = Vec::with_capacity(self.lines.len());
        let mut index = 0usize;
        while index < self.lines.len() {
            if let Some(next) = self.lines.get(index + 1) {
                if matches!(
                    (&self.lines[index], next),
                    (
                        AsmLine::Instr(AsmInstr::Movf { f, d: Dest::W }),
                        AsmLine::Instr(AsmInstr::Movwf(g))
                    ) if f == g
                ) {
                    optimized.push(self.lines[index].clone());
                    index += 2;
                    stats.removed_instructions += 1;
                    stats.self_moves_removed += 1;
                    continue;
                }

                if matches!(
                    (&self.lines[index], next),
                    (AsmLine::Instr(AsmInstr::Movwf(f)), AsmLine::Instr(AsmInstr::Movwf(g))) if f == g
                ) {
                    optimized.push(self.lines[index].clone());
                    index += 2;
                    stats.removed_instructions += 1;
                    stats.duplicate_writes_removed += 1;
                    continue;
                }

                if matches!(
                    (&self.lines[index], next),
                    (AsmLine::Instr(AsmInstr::Bcf { f, b }), AsmLine::Instr(AsmInstr::Bcf { f: g, b: c }))
                        if f == g && b == c
                ) || matches!(
                    (&self.lines[index], next),
                    (AsmLine::Instr(AsmInstr::Bsf { f, b }), AsmLine::Instr(AsmInstr::Bsf { f: g, b: c }))
                        if f == g && b == c
                ) {
                    optimized.push(self.lines[index].clone());
                    index += 2;
                    stats.removed_instructions += 1;
                    stats.duplicate_bit_ops_removed += 1;
                    continue;
                }

                if matches!(
                    (&self.lines[index], next),
                    (AsmLine::Instr(AsmInstr::SetPage(lhs)), AsmLine::Instr(AsmInstr::SetPage(rhs))) if lhs == rhs
                ) {
                    optimized.push(self.lines[index].clone());
                    index += 2;
                    stats.removed_instructions += 1;
                    stats.duplicate_setpages_removed += 1;
                    continue;
                }

                if matches!(
                    (&self.lines[index], next),
                    (AsmLine::Instr(AsmInstr::Movlw(_)), AsmLine::Instr(AsmInstr::Movlw(_)))
                        | (AsmLine::Instr(AsmInstr::Movlw(_)), AsmLine::Instr(AsmInstr::Clrw))
                        | (AsmLine::Instr(AsmInstr::Clrw), AsmLine::Instr(AsmInstr::Movlw(_)))
                        | (AsmLine::Instr(AsmInstr::Clrw), AsmLine::Instr(AsmInstr::Clrw))
                ) {
                    optimized.push(next.clone());
                    index += 2;
                    stats.removed_instructions += 1;
                    stats.overwritten_w_loads_removed += 1;
                    continue;
                }
            }

            optimized.push(self.lines[index].clone());
            index += 1;
        }

        self.lines = optimized;
        stats
    }

    /// Renders assembly lines into the textual `.asm` artifact format.
    pub fn render(&self) -> String {
        let mut output = String::new();
        for line in &self.lines {
            match line {
                AsmLine::Org(addr) => {
                    let _ = writeln!(output, "org 0x{addr:04X}");
                }
                AsmLine::Label(label) => {
                    let _ = writeln!(output, "{label}:");
                }
                AsmLine::Instr(instr) => {
                    let _ = writeln!(output, "  {}", render_instr(instr));
                }
                AsmLine::Comment(text) => {
                    let _ = writeln!(output, "  ; {text}");
                }
            }
        }
        output
    }
}

impl Default for AsmProgram {
    /// Creates an empty assembly program.
    fn default() -> Self {
        Self::new()
    }
}

impl AsmInstr {
    /// Returns the encoded word count for an assembly instruction or pseudo-op.
    pub const fn word_len(&self) -> u16 {
        match self {
            Self::SetPage(_) => 4,
            _ => 1,
        }
    }
}

/// Formats one assembly instruction using the listing/assembly artifact syntax.
pub fn render_instr(instr: &AsmInstr) -> String {
    match instr {
        AsmInstr::Nop => "nop".to_string(),
        AsmInstr::Movlw(value) => format!("movlw 0x{value:02X}"),
        AsmInstr::Movwf(f) => format!("movwf 0x{f:02X}"),
        AsmInstr::Movf { f, d } => format!("movf 0x{f:02X},{}", render_dest(*d)),
        AsmInstr::Clrf(f) => format!("clrf 0x{f:02X}"),
        AsmInstr::Clrw => "clrw".to_string(),
        AsmInstr::Addlw(value) => format!("addlw 0x{value:02X}"),
        AsmInstr::Andlw(value) => format!("andlw 0x{value:02X}"),
        AsmInstr::Iorlw(value) => format!("iorlw 0x{value:02X}"),
        AsmInstr::Xorlw(value) => format!("xorlw 0x{value:02X}"),
        AsmInstr::Addwf { f, d } => format!("addwf 0x{f:02X},{}", render_dest(*d)),
        AsmInstr::Andwf { f, d } => format!("andwf 0x{f:02X},{}", render_dest(*d)),
        AsmInstr::Iorwf { f, d } => format!("iorwf 0x{f:02X},{}", render_dest(*d)),
        AsmInstr::Xorwf { f, d } => format!("xorwf 0x{f:02X},{}", render_dest(*d)),
        AsmInstr::Subwf { f, d } => format!("subwf 0x{f:02X},{}", render_dest(*d)),
        AsmInstr::Rlf { f, d } => format!("rlf 0x{f:02X},{}", render_dest(*d)),
        AsmInstr::Rrf { f, d } => format!("rrf 0x{f:02X},{}", render_dest(*d)),
        AsmInstr::Swapf { f, d } => format!("swapf 0x{f:02X},{}", render_dest(*d)),
        AsmInstr::Bcf { f, b } => format!("bcf 0x{f:02X},{b}"),
        AsmInstr::Bsf { f, b } => format!("bsf 0x{f:02X},{b}"),
        AsmInstr::Btfsc { f, b } => format!("btfsc 0x{f:02X},{b}"),
        AsmInstr::Btfss { f, b } => format!("btfss 0x{f:02X},{b}"),
        AsmInstr::Goto(label) => format!("goto {label}"),
        AsmInstr::Call(label) => format!("call {label}"),
        AsmInstr::Return => "return".to_string(),
        AsmInstr::Retfie => "retfie".to_string(),
        AsmInstr::SetPage(label) => format!("; setpage {label}"),
    }
}

/// Formats a destination selector as the PIC16 assembler expects.
fn render_dest(dest: Dest) -> &'static str {
    match dest {
        Dest::W => "w",
        Dest::F => "f",
    }
}

#[cfg(test)]
mod tests {
    use super::{AsmInstr, AsmLine, AsmProgram, Dest};

    #[test]
    /// Verifies peephole cleanup removes `movf X,w` followed by `movwf X`.
    fn peephole_removes_self_move_roundtrip() {
        let mut program = AsmProgram {
            lines: vec![
                AsmLine::Instr(AsmInstr::Movf { f: 0x20, d: Dest::W }),
                AsmLine::Instr(AsmInstr::Movwf(0x20)),
            ],
        };

        let stats = program.peephole_optimize();
        assert_eq!(program.lines.len(), 1);
        assert_eq!(stats.self_moves_removed, 1);
    }

    #[test]
    /// Verifies adjacent duplicate `movwf` instructions collapse to one store.
    fn peephole_removes_duplicate_movwf() {
        let mut program = AsmProgram {
            lines: vec![
                AsmLine::Instr(AsmInstr::Movwf(0x21)),
                AsmLine::Instr(AsmInstr::Movwf(0x21)),
            ],
        };

        let stats = program.peephole_optimize();
        assert_eq!(program.lines.len(), 1);
        assert_eq!(stats.duplicate_writes_removed, 1);
    }

    #[test]
    /// Verifies repeated `setpage` pseudo-ops for the same target collapse to one.
    fn peephole_removes_duplicate_setpage() {
        let mut program = AsmProgram {
            lines: vec![
                AsmLine::Instr(AsmInstr::SetPage("fn_main".to_string())),
                AsmLine::Instr(AsmInstr::SetPage("fn_main".to_string())),
            ],
        };

        let stats = program.peephole_optimize();
        assert_eq!(program.lines.len(), 1);
        assert_eq!(stats.duplicate_setpages_removed, 1);
    }

    #[test]
    /// Verifies an overwritten W literal load is dropped before the final load.
    fn peephole_removes_overwritten_w_load() {
        let mut program = AsmProgram {
            lines: vec![
                AsmLine::Instr(AsmInstr::Movlw(0x12)),
                AsmLine::Instr(AsmInstr::Movlw(0x34)),
            ],
        };

        let stats = program.peephole_optimize();
        assert_eq!(program.lines.len(), 1);
        assert!(matches!(program.lines[0], AsmLine::Instr(AsmInstr::Movlw(0x34))));
        assert_eq!(stats.overwritten_w_loads_removed, 1);
    }
}
