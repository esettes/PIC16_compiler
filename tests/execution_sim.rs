// SPDX-License-Identifier: GPL-3.0-or-later

use std::collections::BTreeMap;
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
        "pic16f628a",
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
        "pic16f628a",
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
        "pic16f628a",
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
