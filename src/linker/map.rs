use std::fmt::Write;

#[derive(Clone, Debug, Default)]
pub struct MapFile {
    pub code_symbols: Vec<(String, u16)>,
    pub data_symbols: Vec<(String, u16)>,
}

/// Renders the linker map with code and data symbol addresses.
pub fn render_map(map: &MapFile) -> String {
    let mut output = String::new();
    let _ = writeln!(output, "Code Symbols");
    let _ = writeln!(output, "------------");
    for (name, addr) in &map.code_symbols {
        let _ = writeln!(output, "{addr:04X}  {name}");
    }
    let _ = writeln!(output);
    let _ = writeln!(output, "Data Symbols");
    let _ = writeln!(output, "------------");
    for (name, addr) in &map.data_symbols {
        let _ = writeln!(output, "{addr:04X}  {name}");
    }
    output
}
