// SPDX-License-Identifier: GPL-3.0-or-later

use std::fmt::Write;

#[derive(Clone, Debug, Default)]
pub struct MapFile {
    pub resource_lines: Vec<String>,
    pub code_layout_lines: Vec<String>,
    pub code_symbols: Vec<(String, u16)>,
    pub data_symbols: Vec<(String, u16)>,
    pub rom_symbols: Vec<(String, u16)>,
}

/// Renders the linker map with code and data symbol addresses.
pub fn render_map(map: &MapFile) -> String {
    let mut output = String::new();
    if !map.resource_lines.is_empty() {
        let _ = writeln!(output, "Memory Summary");
        let _ = writeln!(output, "--------------");
        for line in &map.resource_lines {
            let _ = writeln!(output, "{line}");
        }
        let _ = writeln!(output);
    }
    if !map.code_layout_lines.is_empty() {
        let _ = writeln!(output, "Code Layout");
        let _ = writeln!(output, "-----------");
        for line in &map.code_layout_lines {
            let _ = writeln!(output, "{line}");
        }
        let _ = writeln!(output);
    }
    render_section(&mut output, "Code Symbols", &map.code_symbols);
    render_grouped(
        &mut output,
        "  User Code",
        true,
        &map.code_symbols,
        |name| !name.starts_with("__rt_") && !name.starts_with("__"),
    );
    render_grouped(
        &mut output,
        "  Runtime Helpers",
        true,
        &map.code_symbols,
        |name| name.starts_with("__rt_"),
    );
    render_grouped(
        &mut output,
        "  Internal / Vectors",
        true,
        &map.code_symbols,
        |name| name.starts_with("__") && !name.starts_with("__rt_"),
    );
    let _ = writeln!(output);
    render_section(&mut output, "Data Symbols", &map.data_symbols);
    render_grouped(
        &mut output,
        "  User Data",
        false,
        &map.data_symbols,
        |name| !name.starts_with("__"),
    );
    render_grouped(
        &mut output,
        "  String Literals",
        false,
        &map.data_symbols,
        |name| name.starts_with("__strlit"),
    );
    render_grouped(
        &mut output,
        "  ABI / Stack",
        false,
        &map.data_symbols,
        |name| {
            name.starts_with("__abi.")
                || name.starts_with("__stack.")
                || name.starts_with("__stack_")
                || name.starts_with("__frame_ptr")
        },
    );
    render_grouped(
        &mut output,
        "  ISR Context",
        false,
        &map.data_symbols,
        |name| name.starts_with("__isr_ctx."),
    );
    let _ = writeln!(output);
    render_section(&mut output, "ROM Symbols", &map.rom_symbols);
    render_grouped(&mut output, "  User ROM", true, &map.rom_symbols, |_| true);
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
fn render_grouped<F>(
    output: &mut String,
    title: &str,
    show_page: bool,
    symbols: &[(String, u16)],
    mut include: F,
) where
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
        if show_page {
            let _ = writeln!(
                output,
                "    {addr:04X}  {name}  page={}",
                control_page(*addr)
            );
        } else {
            let _ = writeln!(output, "    {addr:04X}  {name}");
        }
    }
}

/// Returns the PIC16 program-control page selected by PCLATH<4:3>.
const fn control_page(addr: u16) -> u8 {
    ((addr >> 11) & 0x03) as u8
}
// SPDX-License-Identifier: GPL-3.0-or-later
