use std::fmt::Write;

#[derive(Clone, Debug, Default)]
pub struct MapFile {
    pub code_symbols: Vec<(String, u16)>,
    pub data_symbols: Vec<(String, u16)>,
    pub rom_symbols: Vec<(String, u16)>,
}

/// Renders the linker map with code and data symbol addresses.
pub fn render_map(map: &MapFile) -> String {
    let mut output = String::new();
    render_section(&mut output, "Code Symbols", &map.code_symbols);
    render_grouped(
        &mut output,
        "  User Code",
        &map.code_symbols,
        |name| !name.starts_with("__rt_") && !name.starts_with("__"),
    );
    render_grouped(
        &mut output,
        "  Runtime Helpers",
        &map.code_symbols,
        |name| name.starts_with("__rt_"),
    );
    render_grouped(
        &mut output,
        "  Internal / Vectors",
        &map.code_symbols,
        |name| name.starts_with("__") && !name.starts_with("__rt_"),
    );
    let _ = writeln!(output);
    render_section(&mut output, "Data Symbols", &map.data_symbols);
    render_grouped(
        &mut output,
        "  User Data",
        &map.data_symbols,
        |name| !name.starts_with("__"),
    );
    render_grouped(
        &mut output,
        "  String Literals",
        &map.data_symbols,
        |name| name.starts_with("__strlit"),
    );
    render_grouped(
        &mut output,
        "  ABI / Stack",
        &map.data_symbols,
        |name| name.starts_with("__abi.") || name.starts_with("__stack."),
    );
    render_grouped(
        &mut output,
        "  ISR Context",
        &map.data_symbols,
        |name| name.starts_with("__isr_ctx."),
    );
    let _ = writeln!(output);
    render_section(&mut output, "ROM Symbols", &map.rom_symbols);
    render_grouped(
        &mut output,
        "  User ROM",
        &map.rom_symbols,
        |_| true,
    );
    output
}

/// Renders one top-level map section heading.
fn render_section(output: &mut String, title: &str, symbols: &[(String, u16)]) {
    let _ = writeln!(output, "{title}");
    let _ = writeln!(output, "{}", "-".repeat(title.len()));
    if symbols.is_empty() {
        let _ = writeln!(output, "(none)");
    }
}

/// Renders one filtered group of symbols with indentation for readability.
fn render_grouped<F>(output: &mut String, title: &str, symbols: &[(String, u16)], mut include: F)
where
    F: FnMut(&str) -> bool,
{
    let group = symbols
        .iter()
        .filter(|(name, _)| include(name))
        .collect::<Vec<_>>();
    if group.is_empty() {
        return;
    }
    let _ = writeln!(output, "{title}");
    for (name, addr) in group {
        let _ = writeln!(output, "    {addr:04X}  {name}");
    }
}
