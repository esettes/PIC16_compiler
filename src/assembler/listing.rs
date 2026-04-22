use std::collections::BTreeMap;
use std::fmt::Write;

use crate::backend::pic16::midrange14::asm::AsmProgram;

pub fn render_listing(program: &AsmProgram, words: &BTreeMap<u16, u16>) -> String {
    let mut output = String::new();
    let _ = writeln!(output, "Address  Word");
    let _ = writeln!(output, "-------  ----");
    for (addr, word) in words {
        let _ = writeln!(output, "{addr:04X}     {word:04X}");
    }
    let _ = writeln!(output);
    let _ = writeln!(output, "; Assembly");
    let _ = writeln!(output, "{}", program.render());
    output
}
