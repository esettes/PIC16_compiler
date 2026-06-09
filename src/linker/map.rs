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
        "    integer",
        true,
        &map.code_symbols,
        is_integer_helper,
    );
    render_grouped(
        &mut output,
        "    division",
        true,
        &map.code_symbols,
        is_division_helper,
    );
    render_grouped(
        &mut output,
        "    fixed",
        true,
        &map.code_symbols,
        is_fixed_helper,
    );
    render_grouped(
        &mut output,
        "    float",
        true,
        &map.code_symbols,
        is_float_helper,
    );
    render_grouped(
        &mut output,
        "    math",
        true,
        &map.code_symbols,
        is_math_helper,
    );
    render_grouped(
        &mut output,
        "    conversion",
        true,
        &map.code_symbols,
        is_conversion_helper,
    );
    render_grouped(
        &mut output,
        "    shift",
        true,
        &map.code_symbols,
        is_shift_helper,
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

fn is_fixed_helper(name: &str) -> bool {
    !is_conversion_helper(name)
        && (name.contains("_q8_8")
            || name.contains("_uq8_8")
            || name.contains("_q16_16")
            || name.contains("_uq16_16"))
}

fn is_float_helper(name: &str) -> bool {
    name.starts_with("__rt_f32_") && !is_conversion_helper(name) && !is_math_helper(name)
}

fn is_conversion_helper(name: &str) -> bool {
    name.contains("_to_f32") || name.contains("__rt_f32_to_")
}

fn is_math_helper(name: &str) -> bool {
    matches!(
        name,
        "__rt_f32_fabs"
            | "__rt_f32_trunc"
            | "__rt_f32_floor"
            | "__rt_f32_ceil"
            | "__rt_f32_round"
            | "__rt_f32_sqrt"
            | "__rt_f32_sin"
            | "__rt_f32_cos"
            | "__rt_f32_tan"
            | "__rt_f32_sincos_core"
            | "__rt_math_sin_qwave_table_compact"
            | "__rt_math_sin_qwave_table_balanced"
    )
}

fn is_shift_helper(name: &str) -> bool {
    name.starts_with("__rt_shl") || name.starts_with("__rt_shr")
}

fn is_division_helper(name: &str) -> bool {
    name.starts_with("__rt_div") || name.starts_with("__rt_mod")
}

fn is_integer_helper(name: &str) -> bool {
    name.starts_with("__rt_mul_") && !is_fixed_helper(name)
}
// SPDX-License-Identifier: GPL-3.0-or-later
