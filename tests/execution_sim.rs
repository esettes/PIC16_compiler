// SPDX-License-Identifier: GPL-3.0-or-later

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use pic16cc::backend::pic16::devices::DeviceRegistry;
use pic16cc::cli::{CliCommand, CliOptions, CompileCommand, OptimizationLevel, OutputArtifacts};
use pic16cc::diagnostics::WarningProfile;
use pic16cc::execute;
use pic16cc::sim::{Pic16Core, ProgramImage};

fn repo(path: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(path)
}

fn temp_file(name: &str) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time")
        .as_nanos();
    std::env::temp_dir().join(format!("pic16cc-sim-{stamp}-{name}"))
}

fn compile_source(target: &str, name: &str, source: &str) -> (PathBuf, String) {
    compile_source_with_stack_check(target, name, source, false)
}

fn compile_source_with_stack_check(
    target: &str,
    name: &str,
    source: &str,
    stack_check: bool,
) -> (PathBuf, String) {
    let input = temp_file(name);
    fs::write(&input, source).expect("fixture");
    let output = temp_file("out.hex");
    execute(CliOptions {
        command: CliCommand::Compile(CompileCommand {
            target: target.to_string(),
            input,
            output: output.clone(),
            include_dirs: vec![repo("include")],
            defines: BTreeMap::new(),
            optimization: OptimizationLevel::O2,
            artifacts: OutputArtifacts {
                map: true,
                ..OutputArtifacts::default()
            },
            verbose: false,
            opt_report: false,
            stack_check,
            stack_report: false,
            stack_report_file: None,
            warning_profile: WarningProfile {
                wall: true,
                wextra: true,
                werror: false,
            },
        }),
    })
    .expect("compile source");
    let map = fs::read_to_string(output.with_extension("map")).expect("map");
    (output, map)
}

fn map_symbol_address(map: &str, needle: &str) -> Option<u16> {
    map.lines().find_map(|line| {
        if !line.contains(needle) {
            return None;
        }
        let addr = line.split_whitespace().next()?;
        u16::from_str_radix(addr, 16).ok()
    })
}

fn code_symbol_pages(map: &str) -> BTreeSet<u16> {
    let mut pages = BTreeSet::new();
    let mut in_code = false;
    for raw in map.lines() {
        let line = raw.trim();
        match line {
            "Code Symbols" => {
                in_code = true;
                continue;
            }
            "Data Symbols" | "ROM Symbols" => {
                in_code = false;
                continue;
            }
            _ => {}
        }
        if !in_code {
            continue;
        }
        if let Some(addr) = line
            .split_whitespace()
            .next()
            .and_then(|addr| u16::from_str_radix(addr, 16).ok())
        {
            pages.insert(addr / 0x0800);
        }
    }
    pages
}

fn run_fixture_to_symbol(target: &str, output: &Path, map: &str, stop_symbol: &str) -> Pic16Core {
    let registry = DeviceRegistry::new();
    let device = registry.device(target).expect("device");
    let program = ProgramImage::from_hex_file(output).expect("load hex");
    let halt = map_symbol_address(map, stop_symbol).expect("stop symbol");
    let mut core = Pic16Core::new(device, program);
    core.run_until_pc(halt, 1_000_000).expect("run to halt");
    core
}

fn run_source(target: &str, name: &str, source: &str) -> (Pic16Core, String) {
    let (output, map) = compile_source(target, name, source);
    (run_fixture_to_symbol(target, &output, &map, "__halt"), map)
}

fn symbol_u8(core: &Pic16Core, map: &str, needle: &str) -> u8 {
    let addr = map_symbol_address(map, needle).expect("symbol");
    core.read_data(addr)
}

fn symbol_u16(core: &Pic16Core, map: &str, needle: &str) -> u16 {
    let addr = map_symbol_address(map, needle).expect("symbol");
    core.read_data_u16(addr)
}

fn symbol_u32(core: &Pic16Core, map: &str, needle: &str) -> u32 {
    let addr = map_symbol_address(map, needle).expect("symbol");
    core.read_data_u32(addr)
}

#[test]
fn executes_simple_arithmetic_result() {
    let (core, map) = run_source(
        "pic16f628a",
        "phase19-arith-u8.c",
        r#"
unsigned char result;

void main(void) {
    result = 2 + 3;
}
"#,
    );

    assert_eq!(symbol_u8(&core, &map, "result"), 5);
}

#[test]
fn executes_local_scalar_frame_result() {
    let (core, map) = run_source(
        "pic16f628a",
        "phase19-local-scalar.c",
        r#"
unsigned char result;

void main(void) {
    unsigned char x;
    x = 7;
    result = x;
}
"#,
    );

    assert_eq!(symbol_u8(&core, &map, "result"), 7);
}

#[test]
fn executes_16bit_arithmetic_result() {
    let (core, map) = run_source(
        "pic16f628a",
        "phase19-arith-u16.c",
        r#"
unsigned int result;

void main(void) {
    result = 500;
    result = result + 600;
    result = result - 100;
}
"#,
    );

    assert_eq!(symbol_u16(&core, &map, "result"), 1000);
}

#[test]
fn executes_multiplication_helper_result() {
    let (core, map) = run_source(
        "pic16f628a",
        "phase19-mul.c",
        r#"
unsigned char result;

void main(void) {
    result = 13 * 7;
}
"#,
    );

    assert_eq!(symbol_u8(&core, &map, "result"), 91);
}

#[test]
fn executes_division_modulo_helper_result() {
    let (core, map) = run_source(
        "pic16f628a",
        "phase19-div-mod.c",
        r#"
unsigned char result;

void main(void) {
    result = (22 / 5) * 10 + (22 % 5);
}
"#,
    );

    assert_eq!(symbol_u8(&core, &map, "result"), 42);
}

#[test]
fn executes_if_else_and_loop_control_flow() {
    let (core, map) = run_source(
        "pic16f628a",
        "phase19-loops.c",
        r#"
unsigned char result;

void main(void) {
    unsigned char sum;
    unsigned char i;
    unsigned char j;
    unsigned char k;

    sum = 0;
    i = 0;
    j = 0;
    k = 0;

    if (1) {
        sum = 1;
    } else {
        sum = 99;
    }

    while (i < 3) {
        sum = sum + 1;
        i = i + 1;
    }

    for (j = 0; j < 2; j = j + 1) {
        sum = sum + 2;
    }

    do {
        sum = sum + 3;
        k = k + 1;
    } while (k < 2);

    result = sum;
}
"#,
    );

    assert_eq!(symbol_u8(&core, &map, "result"), 14);
}

#[test]
fn executes_switch_state_machine_result() {
    let (core, map) = run_source(
        "pic16f628a",
        "phase19-switch.c",
        r#"
unsigned char result;

void main(void) {
    unsigned char state;
    state = 2;
    switch (state) {
        case 0:
            result = 1;
            break;
        case 1:
            result = 3;
            break;
        case 2:
            result = 7;
            break;
        default:
            result = 9;
            break;
    }
}
"#,
    );

    assert_eq!(symbol_u8(&core, &map, "result"), 7);
}

#[test]
fn executes_stack_first_three_plus_arg_call_result() {
    let (core, map) = run_source(
        "pic16f628a",
        "phase19-call-args.c",
        r#"
unsigned char result;

unsigned char sum4(unsigned char a, unsigned char b, unsigned char c, unsigned char d) {
    return a + b + c + d;
}

void main(void) {
    result = sum4(1, 2, 3, 4);
}
"#,
    );

    assert_eq!(symbol_u8(&core, &map, "result"), 10);
}

#[test]
fn executes_stack_checked_call_result() {
    let source = r#"
unsigned char result;

unsigned char sum4(unsigned char a, unsigned char b, unsigned char c, unsigned char d) {
    return a + b + c + d;
}

void main(void) {
    result = sum4(1, 2, 3, 4);
}
"#;
    let (output, map) = compile_source_with_stack_check(
        "pic16f628a",
        "phase19-call-args-stack-check.c",
        source,
        true,
    );
    let core = run_fixture_to_symbol("pic16f628a", &output, &map, "__halt");

    assert_eq!(symbol_u8(&core, &map, "result"), 10);
    assert!(map.contains("__stack_overflow_trap"));
}

#[test]
fn executes_single_arg_call_result() {
    let (core, map) = run_source(
        "pic16f628a",
        "phase19-call-one.c",
        r#"
unsigned char result;

unsigned char ident(unsigned char x) {
    return x;
}

void main(void) {
    result = ident(42);
}
"#,
    );

    assert_eq!(symbol_u8(&core, &map, "result"), 42);
}

#[test]
fn executes_nested_call_result() {
    let (core, map) = run_source(
        "pic16f628a",
        "phase19-nested-call.c",
        r#"
unsigned char result;

unsigned char inc(unsigned char x) {
    return x + 1;
}

unsigned char mix(unsigned char x, unsigned char y) {
    return x + y;
}

void main(void) {
    result = mix(inc(4), inc(5));
}
"#,
    );

    assert_eq!(symbol_u8(&core, &map, "result"), 11);
}

#[test]
fn executes_static_local_result() {
    let (core, map) = run_source(
        "pic16f628a",
        "phase19-static-local.c",
        r#"
unsigned char result;

unsigned char bump(void) {
    static unsigned char counter = 3;
    counter = counter + 1;
    return counter;
}

void main(void) {
    result = bump() + bump();
}
"#,
    );

    assert_eq!(symbol_u8(&core, &map, "result"), 9);
}

#[test]
fn executes_function_pointer_dispatch_result() {
    let (core, map) = run_source(
        "pic16f628a",
        "phase19-fnptr.c",
        r#"
typedef unsigned char (*Handler)(unsigned char);

unsigned char result;

unsigned char ident(unsigned char x) {
    return x;
}

unsigned char inc(unsigned char x) {
    return x + 1;
}

Handler table[2] = { ident, inc };

void main(void) {
    result = table[1](41);
}
"#,
    );

    assert_eq!(symbol_u8(&core, &map, "result"), 42);
}

#[test]
fn executes_struct_field_write_read_result() {
    let (core, map) = run_source(
        "pic16f628a",
        "phase19-struct.c",
        r#"
struct Pair {
    unsigned char lo;
    unsigned char hi;
};

unsigned char result;

void main(void) {
    struct Pair p;
    p.lo = 4;
    p.hi = 9;
    result = p.lo + p.hi;
}
"#,
    );

    assert_eq!(symbol_u8(&core, &map, "result"), 13);
}

#[test]
fn executes_union_field_access_result() {
    let (core, map) = run_source(
        "pic16f628a",
        "phase19-union.c",
        r#"
union Value {
    unsigned char byte;
    unsigned int word;
};

unsigned char result;

void main(void) {
    union Value v;
    v.word = 0x1234;
    result = v.byte;
}
"#,
    );

    assert_eq!(symbol_u8(&core, &map, "result"), 0x34);
}

#[test]
fn executes_bitfield_write_read_result() {
    let (core, map) = run_source(
        "pic16f628a",
        "phase19-bitfield.c",
        r#"
struct Flags {
    unsigned char ready:1;
    unsigned char error:1;
    unsigned char mode:2;
};

unsigned char result;

void main(void) {
    struct Flags flags;
    flags.ready = 1;
    flags.error = 0;
    flags.mode = 2;
    result = flags.ready + (flags.mode << 1);
}
"#,
    );

    assert_eq!(symbol_u8(&core, &map, "result"), 5);
}

#[test]
fn executes_multidimensional_array_read_write_result() {
    let (core, map) = run_source(
        "pic16f628a",
        "phase19-matrix.c",
        r#"
unsigned char result;

void main(void) {
    unsigned char matrix[2][3];
    matrix[0][0] = 1;
    matrix[1][2] = 9;
    result = matrix[0][0] + matrix[1][2];
}
"#,
    );

    assert_eq!(symbol_u8(&core, &map, "result"), 10);
}

#[test]
fn executes_pointer_to_pointer_store_result() {
    let (core, map) = run_source(
        "pic16f628a",
        "phase19-ptrptr.c",
        r#"
unsigned char result;

void main(void) {
    unsigned char value;
    unsigned char *ptr;
    unsigned char **pp;

    value = 0;
    ptr = &value;
    pp = &ptr;
    **pp = 42;
    result = value;
}
"#,
    );

    assert_eq!(symbol_u8(&core, &map, "result"), 42);
}

#[test]
fn executes_single_pointer_store_result() {
    let (core, map) = run_source(
        "pic16f628a",
        "phase19-ptr.c",
        r#"
unsigned char result;

void main(void) {
    unsigned char value;
    unsigned char *ptr;

    value = 0;
    ptr = &value;
    *ptr = 42;
    result = value;
}
"#,
    );

    assert_eq!(symbol_u8(&core, &map, "result"), 42);
}

#[test]
fn executes_constant_rom_byte_read_result() {
    let (core, map) = run_source(
        "pic16f628a",
        "phase19-rom-u8-const.c",
        r#"
const __rom unsigned char table[] = { 3, 5, 7, 9 };
unsigned char result;

void main(void) {
    result = table[2];
}
"#,
    );

    assert_eq!(symbol_u8(&core, &map, "result"), 7);
}

#[test]
fn executes_dynamic_rom_byte_read_result() {
    let (core, map) = run_source(
        "pic16f628a",
        "phase19-rom-u8.c",
        r#"
const __rom unsigned char table[] = { 3, 5, 7, 9 };
unsigned char result;

void main(void) {
    unsigned char index;
    index = 2;
    result = table[index];
}
"#,
    );

    assert_eq!(symbol_u8(&core, &map, "result"), 7);
}

#[test]
fn executes_rom_16bit_read_result() {
    let (core, map) = run_source(
        "pic16f628a",
        "phase19-rom-u16.c",
        r#"
const __rom unsigned int table16[] = { 100, 200, 300 };
unsigned int result;

void main(void) {
    unsigned char index;
    index = 2;
    result = table16[index];
}
"#,
    );

    assert_eq!(symbol_u16(&core, &map, "result"), 300);
}

#[test]
fn executes_32bit_add_sub_compare_shift_result() {
    let (core, map) = run_source(
        "pic16f877a",
        "phase21-long-add-shift.c",
        r#"
unsigned long a = 100000UL;
unsigned long b = 250UL;
unsigned long result;

void main(void) {
    unsigned long x;
    x = a + b;
    x = x - 100UL;
    if (x == 100150UL && x > 100000UL && x >= 100150UL) {
        result = x << 1;
        result = result >> 1;
    } else {
        result = 1UL;
    }
}
"#,
    );

    assert_eq!(symbol_u32(&core, &map, "result"), 100_150);
}

#[test]
fn executes_32bit_multiply_helper() {
    let (core, map) = run_source(
        "pic16f628a",
        "phase21-long-mul.c",
        r#"
unsigned long a = 1234UL;
unsigned long b = 17UL;
unsigned long result;

void main(void) {
    result = a * b;
}
"#,
    );

    assert_eq!(symbol_u32(&core, &map, "result"), 20_978);
}

#[test]
fn executes_32bit_divide_helper() {
    let (core, map) = run_source(
        "pic16f877a",
        "phase21-long-div.c",
        r#"
unsigned long c = 100000UL;
unsigned long result;

void main(void) {
    result = c / 25UL;
}
"#,
    );

    assert_eq!(symbol_u32(&core, &map, "result"), 4_000);
}

#[test]
fn executes_32bit_modulo_helper() {
    let (core, map) = run_source(
        "pic16f877a",
        "phase21-long-mod.c",
        r#"
unsigned long c = 100000UL;
unsigned long result;

void main(void) {
    result = c % 97UL;
}
"#,
    );

    assert_eq!(symbol_u32(&core, &map, "result"), 90);
}

#[test]
fn executes_32bit_argument_and_return() {
    let (core, map) = run_source(
        "pic16f628a",
        "phase21-long-call.c",
        r#"
unsigned long result;

unsigned long add32(unsigned long a, unsigned long b) {
    return a + b;
}

void main(void) {
    result = add32(100000UL, 250UL);
}
"#,
    );

    assert_eq!(symbol_u32(&core, &map, "result"), 100_250);
}

#[test]
fn executes_32bit_struct_field_and_startup_init() {
    let (core, map) = run_source(
        "pic16f877a",
        "phase21-long-struct-init.c",
        r#"
struct Counter {
    unsigned long value;
};

struct Counter counter = { 0x01020304UL };
unsigned long result;

void main(void) {
    result = counter.value;
}
"#,
    );

    assert_eq!(symbol_u32(&core, &map, "result"), 0x01020304);
}

#[test]
fn executes_32bit_union_overlay() {
    let (core, map) = run_source(
        "pic16f877a",
        "phase21-long-union.c",
        r#"
union Overlay {
    unsigned long word;
    unsigned char bytes[4];
};

union Overlay overlay;
unsigned long result;

void main(void) {
    overlay.word = 0x11223344UL;
    result = overlay.bytes[0];
}
"#,
    );

    assert_eq!(symbol_u32(&core, &map, "result"), 0x44);
}

#[test]
fn executes_32bit_array_indexing() {
    let (core, map) = run_source(
        "pic16f877a",
        "phase21-long-array.c",
        r#"
unsigned long values[3] = { 10UL, 20UL, 30UL };
unsigned char index;
unsigned long result;

void main(void) {
    index = 2;
    result = values[index];
}
"#,
    );

    assert_eq!(symbol_u32(&core, &map, "result"), 30);
}

#[test]
fn executes_phase22_q8_8_integer_cast_to_fixed() {
    let (core, map) = run_source(
        "pic16f628a",
        "phase22-q8-cast-in.c",
        r#"
__fixed8_8 result;

void main(void) {
    result = (__fixed8_8)3;
}
"#,
    );

    assert_eq!(symbol_u16(&core, &map, "result"), 0x0300);
}

#[test]
fn executes_phase22_q8_8_fixed_cast_to_integer() {
    let (core, map) = run_source(
        "pic16f628a",
        "phase22-q8-cast-out.c",
        r#"
__fixed8_8 value = __q8_8(0x0380);
int result;

void main(void) {
    result = (int)value;
}
"#,
    );

    assert_eq!(symbol_u16(&core, &map, "result"), 3);
}

#[test]
fn executes_phase22_q8_8_add_sub_compare() {
    let (core, map) = run_source(
        "pic16f628a",
        "phase22-q8-add-sub.c",
        r#"
__fixed8_8 a = __q8_8(0x0180);
__fixed8_8 b = __q8_8(0x0240);
__fixed8_8 result;
unsigned char fixed_flag;

void main(void) {
    result = a + b - __q8_8(0x0100);
    if (result > a) {
        fixed_flag = 1;
    } else {
        fixed_flag = 0;
    }
}
"#,
    );

    assert_eq!(symbol_u16(&core, &map, "result"), 0x02C0);
    assert_eq!(symbol_u8(&core, &map, "fixed_flag"), 1);
}

#[test]
fn executes_phase22_q8_8_multiply_helper() {
    let (core, map) = run_source(
        "pic16f877a",
        "phase22-q8-mul.c",
        r#"
__fixed8_8 a = __q8_8(0x0180);
__fixed8_8 b = __q8_8(0x0200);
__fixed8_8 result;

void main(void) {
    result = a * b;
}
"#,
    );

    assert_eq!(symbol_u16(&core, &map, "result"), 0x0300);
}

#[test]
fn executes_phase22_q8_8_divide_helper() {
    let (core, map) = run_source(
        "pic16f877a",
        "phase22-q8-div.c",
        r#"
__fixed8_8 a = __q8_8(0x0300);
__fixed8_8 b = __q8_8(0x0200);
__fixed8_8 result;

void main(void) {
    result = a / b;
}
"#,
    );

    assert_eq!(symbol_u16(&core, &map, "result"), 0x0180);
}

#[test]
fn executes_phase22_q8_8_argument_and_return() {
    let (core, map) = run_source(
        "pic16f628a",
        "phase22-q8-call.c",
        r#"
__fixed8_8 result;

__fixed8_8 twice(__fixed8_8 value) {
    return value + value;
}

void main(void) {
    result = twice(__q8_8(0x0140));
}
"#,
    );

    assert_eq!(symbol_u16(&core, &map, "result"), 0x0280);
}

#[test]
fn executes_phase22_q8_8_struct_and_array_storage() {
    let (core, map) = run_source(
        "pic16f877a",
        "phase22-q8-aggregate.c",
        r#"
struct Sensor {
    __fixed8_8 temperature;
    __ufixed8_8 gain;
};

struct Sensor sensor;
__fixed8_8 table[2];
__fixed8_8 result;

void main(void) {
    sensor.temperature = __q8_8(0x0180);
    sensor.gain = __uq8_8(0x0200);
    table[0] = sensor.temperature;
    table[1] = (__fixed8_8)sensor.gain;
    result = table[0] + table[1];
}
"#,
    );

    assert_eq!(symbol_u16(&core, &map, "result"), 0x0380);
}

#[test]
fn executes_phase22_uq8_8_arithmetic() {
    let (core, map) = run_source(
        "pic16f877a",
        "phase22-uq8-arith.c",
        r#"
__ufixed8_8 a = __uq8_8(0x0300);
__ufixed8_8 b = __uq8_8(0x0200);
__ufixed8_8 sum;
__ufixed8_8 quotient;

void main(void) {
    sum = a + b;
    quotient = a / b;
}
"#,
    );

    assert_eq!(symbol_u16(&core, &map, "sum"), 0x0500);
    assert_eq!(symbol_u16(&core, &map, "quotient"), 0x0180);
}

#[test]
fn executes_phase22_q16_16_add_sub_compare() {
    let (core, map) = run_source(
        "pic16f628a",
        "phase22-q16-add-sub.c",
        r#"
__fixed16_16 a = __q16_16(0x00018000);
__fixed16_16 b = __q16_16(0x00004000);
__fixed16_16 result;
unsigned char fixed_flag;

void main(void) {
    result = a + b - b;
    if (result == a) {
        fixed_flag = 1;
    } else {
        fixed_flag = 0;
    }
}
"#,
    );

    assert_eq!(symbol_u32(&core, &map, "result"), 0x0001_8000);
    assert_eq!(symbol_u8(&core, &map, "fixed_flag"), 1);
}

#[test]
fn executes_phase23_fixed_decimal_literals() {
    let (core, map) = run_source(
        "pic16f628a",
        "phase23-fixed-literals.c",
        r#"
__fixed8_8 q8_result;
__ufixed8_8 uq8_result;
__fixed16_16 q16_result;
__ufixed16_16 uq16_result;

void main(void) {
    q8_result = 1.5q8_8;
    uq8_result = 2.25uq8_8;
    q16_result = 1.5q16_16;
    uq16_result = 0.5uq16_16;
}
"#,
    );

    assert_eq!(symbol_u16(&core, &map, "q8_result"), 0x0180);
    assert_eq!(symbol_u16(&core, &map, "uq8_result"), 0x0240);
    assert_eq!(symbol_u32(&core, &map, "q16_result"), 0x0001_8000);
    assert_eq!(symbol_u32(&core, &map, "uq16_result"), 0x0000_8000);
}

#[test]
fn executes_phase23_fixed_rom_tables() {
    let (core, map) = run_source(
        "pic16f628a",
        "phase23-fixed-rom.c",
        r#"
const __rom __fixed8_8 calibration[] = { 1.0q8_8, 1.5q8_8, 2.0q8_8 };
const __rom __ufixed16_16 gains[] = { 1.0uq16_16, 0.5uq16_16 };
unsigned char index;
__fixed8_8 q8_result;
__ufixed16_16 q16_result;

void main(void) {
    index = 1;
    q8_result = calibration[index];
    q16_result = gains[index];
}
"#,
    );

    assert_eq!(symbol_u16(&core, &map, "q8_result"), 0x0180);
    assert_eq!(symbol_u32(&core, &map, "q16_result"), 0x0000_8000);
}

#[test]
fn executes_phase23_q16_16_multiply_constant_fold() {
    let (core, map) = run_source(
        "pic16f628a",
        "phase23-q16-mul.c",
        r#"
__fixed16_16 result;

void main(void) {
    result = 1.5q16_16 * 2.0q16_16;
}
"#,
    );

    assert_eq!(symbol_u32(&core, &map, "result"), 0x0003_0000);
}

#[test]
fn executes_phase23_q16_16_divide_constant_fold() {
    let (core, map) = run_source(
        "pic16f628a",
        "phase23-q16-div.c",
        r#"
__fixed16_16 result;

void main(void) {
    result = 3.0q16_16 / 2.0q16_16;
}
"#,
    );

    assert_eq!(symbol_u32(&core, &map, "result"), 0x0001_8000);
}

#[test]
fn executes_phase23_signed_q16_16_negative_multiply() {
    let (core, map) = run_source(
        "pic16f628a",
        "phase23-q16-neg-mul.c",
        r#"
__fixed16_16 result;

void main(void) {
    result = -1.5q16_16 * 2.0q16_16;
}
"#,
    );

    assert_eq!(symbol_u32(&core, &map, "result"), 0xFFFD_0000);
}

#[test]
fn executes_phase23_unsigned_uq16_16_multiply_constant_fold() {
    let (core, map) = run_source(
        "pic16f628a",
        "phase23-uq16-mul.c",
        r#"
__ufixed16_16 result;

void main(void) {
    result = 1.5uq16_16 * 2.0uq16_16;
}
"#,
    );

    assert_eq!(symbol_u32(&core, &map, "result"), 0x0003_0000);
}

#[test]
fn executes_phase23_fixed_casts() {
    let (core, map) = run_source(
        "pic16f877a",
        "phase23-fixed-casts.c",
        r#"
__fixed8_8 q8 = 1.5q8_8;
__fixed16_16 q16 = 2.25q16_16;
__fixed16_16 widened;
__fixed8_8 narrowed;
int int_result;
long long_result;
__fixed16_16 from_int;

void main(void) {
    widened = (__fixed16_16)q8;
    narrowed = (__fixed8_8)q16;
    int_result = (int)q8;
    long_result = (long)q16;
    from_int = (__fixed16_16)3;
}
"#,
    );

    assert_eq!(symbol_u32(&core, &map, "widened"), 0x0001_8000);
    assert_eq!(symbol_u16(&core, &map, "narrowed"), 0x0240);
    assert_eq!(symbol_u16(&core, &map, "int_result"), 1);
    assert_eq!(symbol_u32(&core, &map, "long_result"), 2);
    assert_eq!(symbol_u32(&core, &map, "from_int"), 0x0003_0000);
}

#[test]
fn executes_phase24_q16_16_dynamic_multiply() {
    let (core, map) = run_source(
        "pic16f877a",
        "phase24-q16-dynamic-mul.c",
        r#"
__fixed16_16 a = 1.5q16_16;
__fixed16_16 b = 2.0q16_16;
__fixed16_16 c = 0.5q16_16;
__fixed16_16 d = 0.5q16_16;
__fixed16_16 result;
__fixed16_16 quarter;

void main(void) {
    result = a * b;
    quarter = c * d;
}
"#,
    );

    assert_eq!(symbol_u32(&core, &map, "result"), 0x0003_0000);
    assert_eq!(symbol_u32(&core, &map, "quarter"), 0x0000_4000);
}

#[test]
fn executes_phase24_q16_16_dynamic_negative_multiply() {
    let (core, map) = run_source(
        "pic16f877a",
        "phase24-q16-dynamic-neg-mul.c",
        r#"
__fixed16_16 a = -1.5q16_16;
__fixed16_16 b = 2.0q16_16;
__fixed16_16 result;

void main(void) {
    result = a * b;
}
"#,
    );

    assert_eq!(symbol_u32(&core, &map, "result"), 0xFFFD_0000);
}

#[test]
fn executes_phase24_uq16_16_dynamic_multiply() {
    let (core, map) = run_source(
        "pic16f877a",
        "phase24-uq16-dynamic-mul.c",
        r#"
__ufixed16_16 a = 1.5uq16_16;
__ufixed16_16 b = 2.0uq16_16;
__ufixed16_16 result;

void main(void) {
    result = a * b;
}
"#,
    );

    assert_eq!(symbol_u32(&core, &map, "result"), 0x0003_0000);
}

#[test]
fn executes_phase24_q16_16_dynamic_division() {
    let (core, map) = run_source(
        "pic16f877a",
        "phase24-q16-dynamic-div.c",
        r#"
__fixed16_16 a = 3.0q16_16;
__fixed16_16 b = 2.0q16_16;
__fixed16_16 result;

void main(void) {
    result = a / b;
}
"#,
    );

    assert_eq!(symbol_u32(&core, &map, "result"), 0x0001_8000);
}

#[test]
fn executes_phase24_q16_16_dynamic_negative_division() {
    let (core, map) = run_source(
        "pic16f877a",
        "phase24-q16-dynamic-neg-div.c",
        r#"
__fixed16_16 a = -3.0q16_16;
__fixed16_16 b = 2.0q16_16;
__fixed16_16 result;

void main(void) {
    result = a / b;
}
"#,
    );

    assert_eq!(symbol_u32(&core, &map, "result"), 0xFFFE_8000);
}

#[test]
fn executes_phase24_uq16_16_dynamic_division() {
    let (core, map) = run_source(
        "pic16f877a",
        "phase24-uq16-dynamic-div.c",
        r#"
__ufixed16_16 a;
__ufixed16_16 b;
__ufixed16_16 result;

void main(void) {
    a = 3.0uq16_16;
    b = 2.0uq16_16;
    result = a / b;
}
"#,
    );

    assert_eq!(symbol_u32(&core, &map, "result"), 0x0001_8000);
}

#[test]
fn executes_phase30_5_uq16_16_dynamic_division_regressions() {
    for (name, lhs, rhs, expected) in [
        ("3_2", "3.0uq16_16", "2.0uq16_16", 0x0001_8000),
        ("1_2", "1.0uq16_16", "2.0uq16_16", 0x0000_8000),
        ("5_2", "5.0uq16_16", "2.0uq16_16", 0x0002_8000),
        ("half_half", "0.5uq16_16", "0.5uq16_16", 0x0001_0000),
        ("div_zero", "3.0uq16_16", "__uq16_16(0)", 0),
    ] {
        let source = format!(
            r#"
__ufixed16_16 a;
__ufixed16_16 b;
__ufixed16_16 result;

void main(void) {{
    a = {lhs};
    b = {rhs};
    result = a / b;
}}
"#,
        );
        let (core, map) = run_source(
            "pic16f877a",
            &format!("phase30-5-uq16-dynamic-div-{name}.c"),
            &source,
        );

        assert_eq!(symbol_u32(&core, &map, "result"), expected, "{name}");
    }
}

#[test]
fn executes_phase24_q16_16_dynamic_division_by_zero_returns_zero() {
    let (core, map) = run_source(
        "pic16f877a",
        "phase24-q16-dynamic-div-zero.c",
        r#"
__fixed16_16 a = 3.0q16_16;
__fixed16_16 b = __q16_16(0);
__fixed16_16 result;

void main(void) {
    result = a / b;
}
"#,
    );

    assert_eq!(symbol_u32(&core, &map, "result"), 0);
}

#[test]
fn executes_phase24_q16_16_argument_return_operation() {
    let (core, map) = run_source(
        "pic16f877a",
        "phase24-q16-call.c",
        r#"
__fixed16_16 result;

__fixed16_16 scale(__fixed16_16 raw) {
    return raw * 2.0q16_16;
}

void main(void) {
    result = scale(1.5q16_16);
}
"#,
    );

    assert_eq!(symbol_u32(&core, &map, "result"), 0x0003_0000);
}

#[test]
fn executes_phase24_q16_16_struct_field_operation() {
    let (core, map) = run_source(
        "pic16f877a",
        "phase24-q16-struct.c",
        r#"
struct Sensor {
    __fixed16_16 raw;
    __fixed16_16 gain;
};

struct Sensor sensor;
__fixed16_16 result;

void main(void) {
    sensor.raw = 1.5q16_16;
    sensor.gain = 2.0q16_16;
    result = sensor.raw * sensor.gain;
}
"#,
    );

    assert_eq!(symbol_u32(&core, &map, "result"), 0x0003_0000);
}

#[test]
fn executes_phase24_q16_16_array_element_operation() {
    let (core, map) = run_source(
        "pic16f877a",
        "phase24-q16-array.c",
        r#"
__fixed16_16 values[2];
__fixed16_16 result;

void main(void) {
    values[0] = 1.5q16_16;
    values[1] = 2.0q16_16;
    result = values[0] * values[1];
}
"#,
    );

    assert_eq!(symbol_u32(&core, &map, "result"), 0x0003_0000);
}

#[test]
fn executes_phase27_float_literal_raw_encoding() {
    let (core, map) = run_source(
        "pic16f877a",
        "phase27-float-literal.c",
        r#"
float result;

void main(void) {
    result = 1.5f;
}
"#,
    );

    assert_eq!(symbol_u32(&core, &map, "result"), 0x3FC0_0000);
}

#[test]
fn executes_phase27_float_basic_arithmetic() {
    let (core, map) = run_source(
        "pic16f877a",
        "phase27-float-arithmetic.c",
        r#"
float a;
float b;
float sum;

void main(void) {
    a = 1.5f;
    b = 2.0f;
    sum = a + b;
}
"#,
    );

    assert_eq!(symbol_u32(&core, &map, "sum"), 0x4060_0000);
}

#[test]
fn executes_phase27_float_unary_and_comparison() {
    let (core, map) = run_source(
        "pic16f877a",
        "phase27-float-compare.c",
        r#"
float a = 1.5f;
float b = 2.0f;
float neg;
unsigned char eq_flag;
unsigned char lt_flag;
unsigned char gt_flag;

void main(void) {
    neg = -a;
    eq_flag = (1.5f == 1.5f);
    lt_flag = (1.5f < 2.0f);
    gt_flag = (2.0f > 1.5f);
}
"#,
    );

    assert_eq!(symbol_u32(&core, &map, "neg"), 0xBFC0_0000);
    assert_eq!(symbol_u8(&core, &map, "eq_flag"), 1);
    assert_eq!(symbol_u8(&core, &map, "lt_flag"), 1);
    assert_eq!(symbol_u8(&core, &map, "gt_flag"), 1);
}

#[test]
fn executes_phase27_float_constant_casts() {
    let (core, map) = run_source(
        "pic16f877a",
        "phase27-float-casts.c",
        r#"
float from_int;
float from_long;
float from_fixed;
unsigned int to_int;
unsigned long to_long;
__fixed8_8 to_fixed;

void main(void) {
    from_int = (float)3;
    from_long = (float)70000L;
    from_fixed = (float)1.5q8_8;
    to_int = (unsigned int)3.75f;
    to_long = (unsigned long)70000.0f;
    to_fixed = (__fixed8_8)1.5f;
}
"#,
    );

    assert_eq!(symbol_u32(&core, &map, "from_int"), 0x4040_0000);
    assert_eq!(symbol_u32(&core, &map, "from_long"), 0x4788_B800);
    assert_eq!(symbol_u32(&core, &map, "from_fixed"), 0x3FC0_0000);
    assert_eq!(symbol_u16(&core, &map, "to_int"), 3);
    assert_eq!(symbol_u32(&core, &map, "to_long"), 70000);
    assert_eq!(symbol_u16(&core, &map, "to_fixed"), 0x0180);
}

#[test]
fn executes_phase27_float_function_struct_and_array() {
    let (core, map) = run_source(
        "pic16f877a",
        "phase27-float-aggregate-call.c",
        r#"
struct Sensor {
    float raw;
    float gain;
};

struct Sensor sensor;
float values[2];
float result;

void main(void) {
    sensor.raw = 1.5f;
    sensor.gain = 2.0f;
    values[0] = sensor.raw;
    values[1] = sensor.gain;
    result = values[0];
}
"#,
    );

    assert_eq!(symbol_u32(&core, &map, "result"), 0x3FC0_0000);
}

#[test]
fn executes_phase27_float_function_argument_return() {
    let (core, map) = run_source(
        "pic16f877a",
        "phase27-float-call.c",
        r#"
float result;

float echo(float raw) {
    return raw;
}

void main(void) {
    result = echo(2.0f);
}
"#,
    );

    assert_eq!(symbol_u32(&core, &map, "result"), 0x4000_0000);
}

#[test]
fn executes_phase28_dynamic_signed_int_float_round_trip() {
    let (core, map) = run_source(
        "pic16f877a",
        "phase28-float-int-casts.c",
        r#"
int si;
float f;
int si_result;

void main(void) {
    si = -3;
    f = (float)si;
    si_result = (int)f;
}
"#,
    );

    assert_eq!(symbol_u16(&core, &map, "si_result"), 0xFFFD);
}

#[test]
fn executes_phase30_5_dynamic_signed_int_float_round_trip_regressions() {
    for (name, input, expected) in [
        ("zero", "0", 0x0000),
        ("one", "1", 0x0001),
        ("minus_one", "-1", 0xFFFF),
        ("pos", "123", 0x007B),
        ("neg", "-123", 0xFF85),
        ("max", "32767", 0x7FFF),
        ("min", "-32768", 0x8000),
    ] {
        let source = format!(
            r#"
int input;
float temp;
int result;

void main(void) {{
    input = {input};
    temp = (float)input;
    result = (int)temp;
}}
"#,
        );
        let (core, map) = run_source(
            "pic16f877a",
            &format!("phase30-5-float-int-casts-{name}.c"),
            &source,
        );

        assert_eq!(symbol_u16(&core, &map, "result"), expected, "{name}");
    }
}

#[test]
fn executes_phase31_page_crossing_q16_division_helpers() {
    let (core, map) = run_source(
        "pic16f877a",
        "phase31-q16-page-cross.c",
        r#"
__ufixed16_16 ua;
__ufixed16_16 ub;
__ufixed16_16 ur;
__fixed16_16 sa;
__fixed16_16 sb;
__fixed16_16 sr;

void main(void) {
    ua = 5.0uq16_16;
    ub = 2.0uq16_16;
    ur = ua / ub;
    sa = -3.0q16_16;
    sb = 2.0q16_16;
    sr = sa / sb;
}
"#,
    );

    assert_eq!(symbol_u32(&core, &map, "ur"), 0x0002_8000);
    assert_eq!(symbol_u32(&core, &map, "sr"), 0xFFFE_8000);
    assert!(map.contains("page="));
    assert!(code_symbol_pages(&map).len() > 1);
}

#[test]
fn executes_phase31_page_crossing_float_rom_and_dispatcher_paths() {
    let (core, map) = run_source(
        "pic16f877a",
        "phase31-float-rom-fnptr-page-cross.c",
        r#"
const __rom float calibration[] = {
    1.0f,
    1.5f,
    2.0f
};

float value;
unsigned char result;

unsigned char pick(unsigned char raw) {
    return raw + 1;
}

void main(void) {
    unsigned char (*fn)(unsigned char);
    unsigned char index;

    fn = pick;
    index = fn(1);
    value = calibration[index];

    if (value > 1.5f) {
        result = 7;
    } else {
        result = 3;
    }
}
"#,
    );

    assert_eq!(symbol_u8(&core, &map, "result"), 7);
    assert_eq!(symbol_u32(&core, &map, "value"), 0x4000_0000);
    assert!(map.contains("__fp_dispatch"));
    assert!(map.contains("page="));
    assert!(code_symbol_pages(&map).len() > 1);
}

#[test]
fn executes_phase31_stack_trap_layout_with_stack_check() {
    let (output, map) = compile_source_with_stack_check(
        "pic16f877a",
        "phase31-stack-trap-layout.c",
        r#"
unsigned char result;

unsigned char inc(unsigned char value) {
    return value + 1;
}

void main(void) {
    result = inc(6);
}
"#,
        true,
    );
    let core = run_fixture_to_symbol("pic16f877a", &output, &map, "__halt");

    assert_eq!(symbol_u8(&core, &map, "result"), 7);
    assert!(map.contains("__stack_overflow_trap"));
    assert!(map.contains("page="));
}

#[test]
fn executes_phase32_float_comparison_across_pages() {
    let (core, map) = run_source(
        "pic16f877a",
        "phase32-float-layout.c",
        r#"
float lhs;
float rhs;
unsigned char ok;

void main(void) {
    lhs = 3.0f;
    rhs = 2.0f;
    if (lhs > rhs) {
        ok = 1;
    } else {
        ok = 0;
    }
}
"#,
    );

    assert_eq!(symbol_u8(&core, &map, "ok"), 1);
    assert!(map.contains("Code Layout"));
    assert!(code_symbol_pages(&map).len() > 1);
}

#[test]
fn executes_phase32_rom_float_and_fixed_tables_across_pages() {
    let (core, map) = run_source(
        "pic16f877a",
        "phase32-rom-table-layout.c",
        r#"
const __rom float gains[] = { 1.0f, 1.5f, 2.0f };
const __rom __fixed8_8 fixed_gains[] = { 1.0q8_8, 1.5q8_8, 2.0q8_8 };

float fresult;
__fixed8_8 qresult;
unsigned char ok;

void main(void) {
    unsigned char index;

    index = 1;
    fresult = gains[index];
    qresult = fixed_gains[index];
    ok = 1;
}
"#,
    );

    assert_eq!(symbol_u8(&core, &map, "ok"), 1);
    assert_eq!(symbol_u32(&core, &map, "fresult"), 0x3FC0_0000);
    assert_eq!(symbol_u16(&core, &map, "qresult"), 0x0180);
    assert!(map.contains("Code Layout"));
    assert!(map.contains("gains [rom, const"));
    assert!(map.contains("fixed_gains [rom, const"));
}

#[test]
fn executes_phase33_pruned_runtime_regression_paths() {
    let (core, map) = run_source(
        "pic16f877a",
        "phase33-runtime-regression.c",
        r#"
unsigned long ua;
unsigned long ub;
unsigned long uresult;
__fixed16_16 qa;
__fixed16_16 qb;
__fixed16_16 qresult;

void main(void) {
    ua = 100000UL;
    ub = 4UL;
    uresult = ua / ub;
    qa = 3.0q16_16;
    qb = 2.0q16_16;
    qresult = qa / qb;
}
"#,
    );

    assert_eq!(symbol_u32(&core, &map, "uresult"), 25000);
    assert_eq!(symbol_u32(&core, &map, "qresult"), 0x0001_8000);
    assert!(map.contains("__rt_div_u32"));
    assert!(map.contains("__rt_div_q16_16"));
}

#[test]
fn executes_phase28_dynamic_unsigned_int_float_round_trip() {
    let (core, map) = run_source(
        "pic16f877a",
        "phase28-float-uint-casts.c",
        r#"
unsigned int phase28_uint_value;
float phase28_float_value;
unsigned int phase28_uint_result;

void main(void) {
    phase28_uint_value = 5;
    phase28_float_value = (float)phase28_uint_value;
    phase28_uint_result = (unsigned int)phase28_float_value;
}
"#,
    );

    assert_eq!(symbol_u16(&core, &map, "phase28_uint_result"), 5);
}

#[test]
fn executes_phase28_dynamic_fixed_float_round_trips() {
    let (core, map) = run_source(
        "pic16f877a",
        "phase28-float-fixed-casts.c",
        r#"
__fixed16_16 q16;
float f;
__fixed16_16 q16_result;

void main(void) {
    q16 = -2.25q16_16;
    f = (float)q16;
    q16_result = (__fixed16_16)f;
}
"#,
    );

    assert_eq!(symbol_u32(&core, &map, "q16_result"), 0xFFFD_C000);
}

#[test]
fn executes_phase28_dynamic_q8_float_round_trip() {
    let (core, map) = run_source(
        "pic16f877a",
        "phase28-float-q8-casts.c",
        r#"
__fixed8_8 q8;
float f;
__fixed8_8 q8_result;

void main(void) {
    q8 = 1.5q8_8;
    f = (float)q8;
    q8_result = (__fixed8_8)f;
}
"#,
    );

    assert_eq!(symbol_u16(&core, &map, "q8_result"), 0x0180);
}

#[test]
fn executes_phase28_float_pointer_and_union_storage() {
    let (core, map) = run_source(
        "pic16f877a",
        "phase28-float-pointer-union.c",
        r#"
union FloatRaw {
    float f;
    unsigned long raw;
};

float value;
float copied;
float *ptr;
union FloatRaw overlay;
unsigned long raw_result;

void main(void) {
    value = -1.5f;
    ptr = &value;
    copied = *ptr;
    overlay.f = copied;
    raw_result = overlay.raw;
}
"#,
    );

    assert_eq!(symbol_u32(&core, &map, "raw_result"), 0xBFC0_0000);
}

#[test]
fn executes_phase29_dynamic_float_comparisons_and_control_flow() {
    let (core, map) = run_source(
        "pic16f877a",
        "phase29-float-comparisons.c",
        r#"
float a;
float b;
unsigned char result;
unsigned char loop_count;

void main(void) {
    a = 1.0f;
    b = 1.0f;
    if (a == b) { result = result + 1; }

    b = 2.0f;
    if (a != b) { result = result + 1; }
    a = 1.5f;
    if (a < b) { result = result + 1; }
    if (b <= b) { result = result + 1; }
    a = 3.0f;
    if (a > b) { result = result + 1; }
    if (a >= a) { result = result + 1; }
    a = -1.0f;
    if (a < b) { result = result + 1; }
    b = -4.0f;
    if (a > b) { result = result + 1; }

    a = 30.5f;
    b = 28.0f;
    if (a > b) { result = result + 1; }

    a = 0.0f;
    b = 1.0f;
    while (a < b) {
        loop_count = loop_count + 1;
        a = 2.0f;
    }
}
"#,
    );

    assert_eq!(symbol_u8(&core, &map, "result"), 9);
    assert_eq!(symbol_u8(&core, &map, "loop_count"), 1);
}

#[test]
fn executes_phase29_dynamic_i32_to_float_casts() {
    let (core, map) = run_source(
        "pic16f877a",
        "phase29-i32-to-float.c",
        r#"
long large_signed;
long negative_signed;
unsigned long large_unsigned;
float signed_result;
float negative_result;
float unsigned_result;

void main(void) {
    large_signed = 100000L;
    negative_signed = -1000L;
    large_unsigned = 100000UL;
    signed_result = (float)large_signed;
    negative_result = (float)negative_signed;
    unsigned_result = (float)large_unsigned;
}
"#,
    );

    assert_eq!(symbol_u32(&core, &map, "signed_result"), 0x47C3_5000);
    assert_eq!(symbol_u32(&core, &map, "negative_result"), 0xC47A_0000);
    assert_eq!(symbol_u32(&core, &map, "unsigned_result"), 0x47C3_5000);
}

#[test]
fn executes_phase29_dynamic_float_to_i32_casts() {
    let (core, map) = run_source(
        "pic16f877a",
        "phase29-float-to-i32.c",
        r#"
float positive_value;
float negative_value;
float unsigned_value;
long positive_result;
long negative_result;
unsigned long unsigned_result;

void main(void) {
    positive_value = 1000.75f;
    negative_value = -1000.75f;
    unsigned_value = 1000.75f;
    positive_result = (long)positive_value;
    negative_result = (long)negative_value;
    unsigned_result = (unsigned long)unsigned_value;
}
"#,
    );

    assert_eq!(symbol_u32(&core, &map, "positive_result"), 1000);
    assert_eq!(symbol_u32(&core, &map, "negative_result"), 0xFFFF_FC18);
    assert_eq!(symbol_u32(&core, &map, "unsigned_result"), 1000);
}

#[test]
fn executes_phase29_negative_float_to_unsigned_long_returns_zero() {
    let (core, map) = run_source(
        "pic16f877a",
        "phase29-negative-float-to-u32.c",
        r#"
float negative_value;
unsigned long result;

void main(void) {
    negative_value = -1.0f;
    result = (unsigned long)negative_value;
}
"#,
    );

    assert_eq!(symbol_u32(&core, &map, "result"), 0);
}

#[test]
fn executes_phase30_rom_float_reads() {
    let (core, map) = run_source(
        "pic16f877a",
        "phase30-rom-float-reads.c",
        r#"
const __rom float calibration[] = { 1.0f, 1.5f, 2.0f };

struct Sample {
    float gain;
};

unsigned char index;
float first;
float selected;
float from_function;
struct Sample sample;

float read_gain(unsigned char slot) {
    return calibration[slot];
}

void main(void) {
    index = 1;
    first = calibration[0];
    selected = calibration[index];
    from_function = read_gain(index);
    sample.gain = calibration[2];
}
"#,
    );

    assert_eq!(symbol_u32(&core, &map, "first"), 0x3F80_0000);
    assert_eq!(symbol_u32(&core, &map, "selected"), 0x3FC0_0000);
    assert_eq!(symbol_u32(&core, &map, "from_function"), 0x3FC0_0000);
    assert_eq!(symbol_u32(&core, &map, "sample"), 0x4000_0000);
}

#[test]
fn executes_phase30_rom_float_read_then_arithmetic() {
    let (core, map) = run_source(
        "pic16f877a",
        "phase30-rom-float-arith.c",
        r#"
const __rom float gains[] = { 1.5f, 2.0f };
float lhs;
float rhs;
float result;

void main(void) {
    lhs = gains[0];
    rhs = gains[1];
    result = lhs + rhs;
}
"#,
    );

    assert_eq!(symbol_u32(&core, &map, "result"), 0x4060_0000);
}

#[test]
fn executes_phase30_rom_float_read_then_comparison() {
    let (core, map) = run_source(
        "pic16f877a",
        "phase30-rom-float-compare.c",
        r#"
const __rom float thresholds[] = { 1.5f, 2.0f };
float low;
float high;
unsigned char alarm;

void main(void) {
    low = thresholds[0];
    high = thresholds[1];
    if (high > low) {
        alarm = 1;
    } else {
        alarm = 0;
    }
}
"#,
    );

    assert_eq!(symbol_u8(&core, &map, "alarm"), 1);
}

#[test]
fn executes_phase30_ram_float_static_initializers() {
    let (core, map) = run_source(
        "pic16f877a",
        "phase30-float-static-init.c",
        r#"
float global_value = 1.5f;
float values[] = { 1.0f, 2.0f };
float from_array;
float from_static;

void main(void) {
    static float local_gain = 2.0f;
    from_array = values[1];
    from_static = local_gain;
}
"#,
    );

    assert_eq!(symbol_u32(&core, &map, "global_value"), 0x3FC0_0000);
    assert_eq!(symbol_u32(&core, &map, "from_array"), 0x4000_0000);
    assert_eq!(symbol_u32(&core, &map, "from_static"), 0x4000_0000);
}
