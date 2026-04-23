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
