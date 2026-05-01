use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use pic16cc::cli::{CliCommand, CliOptions, CompileCommand, OptimizationLevel, OutputArtifacts};
use pic16cc::diagnostics::WarningProfile;
use pic16cc::execute;

/// Resolves a repository-relative path inside the test workspace.
fn repo(path: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(path)
}

/// Creates a unique temporary file path for one test artifact.
fn temp_file(name: &str) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time")
        .as_nanos();
    std::env::temp_dir().join(format!("pic16cc-{stamp}-{name}"))
}

/// Creates a unique temporary directory path for CLI output.
fn temp_dir_path(name: &str) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time")
        .as_nanos();
    std::env::temp_dir().join(format!("picc-{stamp}-{name}"))
}

/// Returns the built CLI path for integration tests.
fn picc_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_picc"))
}

/// Compiles one input file with artifact dumps enabled and returns the HEX path.
fn compile_input(target: &str, input: PathBuf) -> PathBuf {
    let output = temp_file("out.hex");
    let options = CliOptions {
        command: CliCommand::Compile(CompileCommand {
            target: target.to_string(),
            input,
            output: output.clone(),
            include_dirs: vec![repo("include")],
            defines: BTreeMap::new(),
            optimization: OptimizationLevel::O2,
            artifacts: OutputArtifacts {
                emit_ast: true,
                emit_ir: true,
                emit_asm: true,
                map: true,
                list_file: true,
                ..OutputArtifacts::default()
            },
            verbose: false,
            opt_report: false,
            warning_profile: WarningProfile {
                wall: true,
                wextra: true,
                werror: false,
            },
        }),
    };
    execute(options).expect("compile example");
    output
}

/// Compiles one checked-in example file for the requested target.
fn compile_example(target: &str, input: &str) -> PathBuf {
    compile_input(target, repo(input))
}

/// Writes source text to a temporary file and compiles it like a user input.
fn compile_source(target: &str, name: &str, source: &str) -> PathBuf {
    let input = temp_file(name);
    fs::write(&input, source).expect("fixture");
    compile_input(target, input)
}

/// Compiles one temporary source using an explicit optimization level.
fn compile_source_with_optimization(
    target: &str,
    name: &str,
    source: &str,
    optimization: OptimizationLevel,
) -> PathBuf {
    let input = temp_file(name);
    fs::write(&input, source).expect("fixture");
    let output = temp_file("opt.hex");
    execute(CliOptions {
        command: CliCommand::Compile(CompileCommand {
            target: target.to_string(),
            input,
            output: output.clone(),
            include_dirs: vec![repo("include")],
            defines: BTreeMap::new(),
            optimization,
            artifacts: OutputArtifacts {
                emit_asm: true,
                map: true,
                list_file: true,
                ..OutputArtifacts::default()
            },
            verbose: false,
            opt_report: false,
            warning_profile: WarningProfile {
                wall: true,
                wextra: true,
                werror: false,
            },
        }),
    })
    .expect("compile fixture");
    output
}

/// Compiles one temporary source expecting a diagnostic failure and returns the rendered error text.
fn compile_error(target: &str, name: &str, source: &str) -> String {
    let input = temp_file(name);
    fs::write(&input, source).expect("fixture");
    let error = execute(CliOptions {
        command: CliCommand::Compile(CompileCommand {
            target: target.to_string(),
            input,
            output: temp_file("error.hex"),
            include_dirs: vec![repo("include")],
            defines: BTreeMap::new(),
            optimization: OptimizationLevel::O0,
            artifacts: OutputArtifacts::default(),
            verbose: false,
            opt_report: false,
            warning_profile: WarningProfile::default(),
        }),
    })
    .expect_err("must fail");
    format!("{error}")
}

/// Compiles one source file path using a custom warning profile.
fn compile_path_with_profile(
    target: &str,
    input: PathBuf,
    warning_profile: WarningProfile,
) -> Result<PathBuf, String> {
    let output = temp_file("profile.hex");
    let result = execute(CliOptions {
        command: CliCommand::Compile(CompileCommand {
            target: target.to_string(),
            input,
            output: output.clone(),
            include_dirs: vec![repo("include")],
            defines: BTreeMap::new(),
            optimization: OptimizationLevel::O2,
            artifacts: OutputArtifacts::default(),
            verbose: false,
            opt_report: false,
            warning_profile,
        }),
    });

    match result {
        Ok(_) => Ok(output),
        Err(error) => Err(format!("{error}")),
    }
}

/// Compiles temporary source text using a custom warning profile.
fn compile_source_with_profile(
    target: &str,
    name: &str,
    source: &str,
    warning_profile: WarningProfile,
) -> Result<PathBuf, String> {
    let input = temp_file(name);
    fs::write(&input, source).expect("fixture");
    compile_path_with_profile(target, input, warning_profile)
}

/// Compiles a checked-in example source with a custom warning profile.
fn compile_example_with_profile(
    target: &str,
    input: &str,
    warning_profile: WarningProfile,
) -> Result<PathBuf, String> {
    compile_path_with_profile(target, repo(input), warning_profile)
}

/// Compiles one checked-in example through the built `picc` CLI under strict warnings.
fn compile_example_via_picc_cli(target: &str, input: &str) -> PathBuf {
    let out_dir = temp_dir_path("phase8-cli");
    fs::create_dir_all(&out_dir).expect("out dir");
    let stem = Path::new(input)
        .file_stem()
        .and_then(|name| name.to_str())
        .expect("example stem");
    let out_hex = out_dir.join(format!("{stem}.hex"));
    let output = Command::new(picc_bin())
        .current_dir(repo("."))
        .args([
            "--target",
            target,
            "-Wall",
            "-Wextra",
            "-Werror",
            "-O2",
            "-I",
            "include",
            "--map",
            "--list-file",
        ])
        .arg("-o")
        .arg(&out_hex)
        .arg(input)
        .output()
        .expect("run picc");

    if !output.status.success() {
        panic!(
            "picc failed for {input}: stdout={:?} stderr={:?}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    out_hex
}

/// Returns a strict warning profile equivalent to `-Wall -Wextra -Werror`.
fn strict_warnings() -> WarningProfile {
    WarningProfile {
        wall: true,
        wextra: true,
        werror: true,
    }
}

/// Checks that the generated HEX includes both config data and the EOF record.
fn assert_hex_is_programmable(output: &Path) {
    let hex = fs::read_to_string(output).expect("hex");
    assert!(hex.contains(":02400E00"));
    assert!(hex.contains(":00000001FF"));
}

/// Reads one side artifact generated next to the compiled HEX output.
fn read_artifact(output: &Path, extension: &str) -> String {
    fs::read_to_string(output.with_extension(extension)).expect("artifact")
}

/// Parses the emitted Intel HEX file into a byte-addressed map for spot checks.
fn read_hex_bytes(output: &Path) -> BTreeMap<u16, u8> {
    let mut bytes = BTreeMap::new();
    for line in fs::read_to_string(output).expect("hex").lines() {
        if !line.starts_with(':') || line.len() < 11 {
            continue;
        }
        let len = u8::from_str_radix(&line[1..3], 16).expect("len") as usize;
        let addr = u16::from_str_radix(&line[3..7], 16).expect("addr");
        let kind = u8::from_str_radix(&line[7..9], 16).expect("kind");
        if kind != 0 {
            continue;
        }
        for index in 0..len {
            let start = 9 + index * 2;
            let byte = u8::from_str_radix(&line[start..start + 2], 16).expect("byte");
            bytes.insert(addr + index as u16, byte);
        }
    }
    bytes
}

/// Finds one symbol address in a rendered map file by matching a readable symbol name fragment.
fn map_symbol_address(map: &str, needle: &str) -> Option<u16> {
    map.lines().find_map(|line| {
        if !line.contains(needle) {
            return None;
        }
        let addr = line.split_whitespace().next()?;
        u16::from_str_radix(addr, 16).ok()
    })
}

/// Counts non-overlapping substring matches in one artifact string.
fn count_occurrences(haystack: &str, needle: &str) -> usize {
    haystack.match_indices(needle).count()
}

/// Verifies Phase 4 artifacts include stack metadata in asm/map/listing outputs.
fn assert_phase4_stack_metadata(output: &Path) {
    let asm = read_artifact(output, "asm");
    let map = read_artifact(output, "map");
    let listing = read_artifact(output, "lst");

    assert!(asm.contains("frame args="));
    assert!(asm.contains("stack base="));
    assert!(map.contains("__abi.stack_ptr.lo"));
    assert!(map.contains("__abi.frame_ptr.lo"));
    assert!(map.contains("__stack.base"));
    assert!(map.contains("__stack.end"));
    assert!(listing.contains("frame args="));
}

/// Verifies Phase 5 helper artifacts expose one runtime helper in asm/map/listing outputs.
fn assert_phase5_helper_artifacts(output: &Path, helper: &str) {
    let asm = read_artifact(output, "asm");
    let map = read_artifact(output, "map");
    let listing = read_artifact(output, "lst");

    assert!(asm.contains(&format!("call {helper}")));
    assert!(asm.contains(&format!("{helper}:")));
    assert!(map.contains(helper));
    assert!(listing.contains(helper));
}

/// Verifies Phase 6 artifacts expose the interrupt vector, ISR symbol, and saved-context slots.
fn assert_phase6_interrupt_artifacts(output: &Path, isr_symbol: &str) {
    let asm = read_artifact(output, "asm");
    let map = read_artifact(output, "map");
    let listing = read_artifact(output, "lst");

    assert!(asm.contains("org 0x0004"));
    assert!(asm.contains("__interrupt_vector:"));
    assert!(asm.contains("goto __interrupt_dispatch"));
    assert!(asm.contains(&format!("{isr_symbol}:")));
    assert!(asm.contains("retfie"));
    assert!(map.contains("__interrupt_vector"));
    assert!(map.contains("__isr_ctx.w"));
    assert!(map.contains("__isr_ctx.status"));
    assert!(map.contains("__isr_ctx.stack_ptr.lo"));
    assert!(listing.contains("__interrupt_vector"));
    assert!(listing.contains("retfie"));
}

#[test]
/// Verifies the original PIC16F628A blink example still compiles successfully.
fn compiles_pic16f628a_blink() {
    let output = compile_example("pic16f628a", "examples/pic16f628a/blink.c");
    assert_hex_is_programmable(&output);
    assert!(read_artifact(&output, "map").contains("Code Symbols"));
    assert!(read_artifact(&output, "lst").contains("Assembly"));
}

#[test]
/// Verifies the original PIC16F877A blink example still emits assembly.
fn compiles_pic16f877a_blink() {
    let output = compile_example("pic16f877a", "examples/pic16f877a/blink.c");
    let asm = read_artifact(&output, "asm");
    assert!(asm.contains("fn_main"));
    assert!(asm.contains("movwf"));
}

#[test]
/// Verifies unsigned 16-bit arithmetic and relational lowering on PIC16F628A.
fn compiles_unsigned_16bit_phase2_example() {
    let output = compile_example("pic16f628a", "examples/pic16f628a/arith16.c");
    let ir = read_artifact(&output, "ir");
    let asm = read_artifact(&output, "asm");
    let map = read_artifact(&output, "map");

    assert_hex_is_programmable(&output);
    assert!(ir.contains("Less"));
    assert!(ir.contains("GreaterEqual"));
    assert!(asm.contains("fn_add16"));
    assert!(asm.contains("call fn_add16"));
    assert!(asm.contains("addwf"));
    assert!(asm.contains("subwf"));
    assert!(map.contains("threshold"));
    assert!(map.contains("counter"));
}

#[test]
/// Verifies signed 16-bit arithmetic and relational lowering on PIC16F877A.
fn compiles_signed_16bit_phase2_example() {
    let output = compile_example("pic16f877a", "examples/pic16f877a/compare16.c");
    let ir = read_artifact(&output, "ir");
    let asm = read_artifact(&output, "asm");

    assert_hex_is_programmable(&output);
    assert!(ir.contains("Less"));
    assert!(ir.contains("LessEqual"));
    assert!(ir.contains("Greater"));
    assert!(asm.contains("fn_adjust16"));
    assert!(asm.contains("call fn_adjust16"));
    assert!(asm.contains("xorwf"));
    assert!(asm.contains("btfss"));
}

#[test]
/// Verifies byte-array decay, indexing, and indirect loads/stores lower to PIC16 assembly.
fn compiles_phase3_byte_array_example() {
    let output = compile_example("pic16f628a", "examples/pic16f628a/array_fill.c");
    let ir = read_artifact(&output, "ir");
    let asm = read_artifact(&output, "asm");
    let map = read_artifact(&output, "map");

    assert_hex_is_programmable(&output);
    assert!(ir.contains("= &s"));
    assert!(ir.contains("= *"));
    assert!(ir.contains("*t"));
    assert!(asm.contains("movwf 0x04"));
    assert!(asm.contains("movf 0x00,w"));
    assert!(asm.contains("movwf 0x00"));
    assert!(map.contains("shadow"));
    assert!(map.contains("total"));
}

#[test]
/// Verifies 16-bit arrays, pointer equality, and indirect SFR writes compile end to end.
fn compiles_phase3_word_pointer_example() {
    let output = compile_example("pic16f877a", "examples/pic16f877a/pointer16.c");
    let ir = read_artifact(&output, "ir");
    let asm = read_artifact(&output, "asm");
    let map = read_artifact(&output, "map");

    assert_hex_is_programmable(&output);
    assert!(ir.contains("= &s"));
    assert!(ir.contains("= *"));
    assert!(ir.contains("Equal"));
    assert!(asm.contains("movwf 0x04"));
    assert!(asm.contains("movf 0x00,w"));
    assert!(asm.contains("movwf 0x00"));
    assert!(map.contains("words"));
    assert!(map.contains("mirror"));
}

#[test]
/// Verifies the Phase 4 stack ABI handles 3+ arguments and nested calls on PIC16F628A.
fn compiles_phase4_stack_abi_example() {
    let output = compile_example("pic16f628a", "examples/pic16f628a/stack_abi.c");
    let asm = read_artifact(&output, "asm");
    let map = read_artifact(&output, "map");

    assert_hex_is_programmable(&output);
    assert_phase4_stack_metadata(&output);
    assert!(asm.contains("stack base="));
    assert!(asm.contains("call fn_sum4"));
    assert!(asm.contains("call fn_build_local"));
    assert!(asm.contains("call fn_sum_bytes"));
    assert!(map.contains("final_value"));
}

#[test]
/// Verifies the Phase 4 frame model handles deeper non-recursive call chains on PIC16F877A.
fn compiles_phase4_call_chain_example() {
    let output = compile_example("pic16f877a", "examples/pic16f877a/call_chain.c");
    let asm = read_artifact(&output, "asm");
    let map = read_artifact(&output, "map");

    assert_hex_is_programmable(&output);
    assert_phase4_stack_metadata(&output);
    assert!(asm.contains("stack base="));
    assert!(asm.contains("call fn_top_sum"));
    assert!(asm.contains("call fn_middle_sum"));
    assert!(asm.contains("call fn_leaf_sum"));
    assert!(map.contains("latest"));
}

#[test]
/// Verifies unsigned 8-bit multiplication lowers through the Phase 5 helper path.
fn compiles_phase5_mul8_fixture() {
    let output = compile_source(
        "pic16f628a",
        "mul8.c",
        "\
#include <pic16/pic16f628a.h>
unsigned char mul8(unsigned char a, unsigned char b) {
    return a * b;
}
void main(void) {
    TRISB = 0x00;
    PORTB = mul8(6, 7);
}
",
    );

    assert_hex_is_programmable(&output);
    assert_phase5_helper_artifacts(&output, "__rt_mul_u8");
}

#[test]
/// Verifies unsigned 16-bit multiplication lowers end to end through runtime helpers.
fn compiles_phase5_mul16_example() {
    let output = compile_example("pic16f877a", "examples/pic16f877a/mul16.c");

    assert_hex_is_programmable(&output);
    assert_phase4_stack_metadata(&output);
    assert_phase5_helper_artifacts(&output, "__rt_mul_u16");
}

#[test]
/// Verifies signed 16-bit division lowers end to end through runtime helpers.
fn compiles_phase5_div16_example() {
    let output = compile_example("pic16f877a", "examples/pic16f877a/div16.c");

    assert_hex_is_programmable(&output);
    assert_phase4_stack_metadata(&output);
    assert_phase5_helper_artifacts(&output, "__rt_div_i16");
}

#[test]
/// Verifies unsigned 16-bit modulo lowers end to end through runtime helpers.
fn compiles_phase5_mod16_example() {
    let output = compile_example("pic16f877a", "examples/pic16f877a/mod16.c");

    assert_hex_is_programmable(&output);
    assert_phase4_stack_metadata(&output);
    assert_phase5_helper_artifacts(&output, "__rt_mod_u16");
}

#[test]
/// Verifies mixed inline/runtime shift lowering emits only the dynamic helper path.
fn compiles_phase5_shift_mix_example() {
    let output = compile_example("pic16f877a", "examples/pic16f877a/shift_mix.c");
    let asm = read_artifact(&output, "asm");

    assert_hex_is_programmable(&output);
    assert_phase4_stack_metadata(&output);
    assert_phase5_helper_artifacts(&output, "__rt_shr_u16");
    assert!(!asm.contains("call __rt_shl16"));
}

#[test]
/// Verifies one expression tree can combine multiple runtime helpers safely.
fn compiles_phase5_expression_example() {
    let output = compile_example("pic16f877a", "examples/pic16f877a/expression_test.c");
    let asm = read_artifact(&output, "asm");

    assert_hex_is_programmable(&output);
    assert_phase4_stack_metadata(&output);
    assert!(asm.contains("call __rt_mul_u16"));
    assert!(asm.contains("call __rt_div_u16"));
    assert!(asm.contains("call __rt_mod_u16"));
}

#[test]
/// Verifies the PIC16F628A timer ISR example emits the interrupt vector and `retfie`.
fn compiles_phase6_pic16f628a_timer_interrupt_example() {
    let output = compile_example("pic16f628a", "examples/pic16f628a/timer_interrupt.c");

    assert_hex_is_programmable(&output);
    assert_phase4_stack_metadata(&output);
    assert_phase6_interrupt_artifacts(&output, "fn_isr");
}

#[test]
/// Verifies the PIC16F877A timer ISR example emits the interrupt vector and saved context.
fn compiles_phase6_pic16f877a_timer_interrupt_example() {
    let output = compile_example("pic16f877a", "examples/pic16f877a/timer_interrupt.c");

    assert_hex_is_programmable(&output);
    assert_phase4_stack_metadata(&output);
    assert_phase6_interrupt_artifacts(&output, "fn_isr");
}

#[test]
/// Verifies the PIC16F877A GPIO ISR example compiles with the same Phase 6 vector shape.
fn compiles_phase6_pic16f877a_gpio_interrupt_example() {
    let output = compile_example("pic16f877a", "examples/pic16f877a/gpio_interrupt.c");

    assert_hex_is_programmable(&output);
    assert_phase4_stack_metadata(&output);
    assert_phase6_interrupt_artifacts(&output, "fn_isr");
}

#[test]
/// Verifies pointer arguments, pointer returns, and local array decay through a fixture.
fn compiles_phase3_pointer_return_fixture() {
    let output = compile_source(
        "pic16f628a",
        "pointer-return.c",
        "\
#include <pic16/pic16f628a.h>
unsigned char bytes[2];
/** Returns the same pointer passed by the caller. */
unsigned char *pick(unsigned char *ptr) {
    return ptr;
}
/** Exercises pointer arguments, returns, equality, and indirect loads. */
void main(void) {
    unsigned char *cursor = pick(bytes);
    TRISB = 0x00;
    PORTB = 0x00;
    cursor[0] = 0x11;
    cursor[1] = 0x22;
    if (cursor == bytes) {
        PORTB = cursor[0];
    }
}
",
    );

    let ir = read_artifact(&output, "ir");
    let asm = read_artifact(&output, "asm");
    assert_hex_is_programmable(&output);
    assert!(ir.contains("call f"));
    assert!(ir.contains("Equal"));
    assert!(asm.contains("call fn_pick"));
    assert!(asm.contains("movwf 0x04"));
}

#[test]
/// Verifies a five-argument call compiles through the Phase 4 caller-pushed stack ABI.
fn compiles_phase4_five_argument_fixture() {
    let output = compile_source(
        "pic16f628a",
        "sum5.c",
        "\
#include <pic16/pic16f628a.h>
/** Returns the sum of five arguments through the Phase 4 ABI. */
unsigned int sum5(unsigned int a, unsigned int b, unsigned int c, unsigned int d, unsigned int e) {
    return a + b + c + d + e;
}
/** Exercises a five-argument call and 16-bit return handling. */
void main(void) {
    unsigned int value = sum5(1, 2, 3, 4, 5);
    TRISB = 0x00;
    if (value >= 15) {
        PORTB = 0x77;
    }
}
",
    );

    let asm = read_artifact(&output, "asm");
    assert_hex_is_programmable(&output);
    assert_phase4_stack_metadata(&output);
    assert!(asm.contains("call fn_sum5"));
    assert!(asm.contains("stack base="));
}

#[test]
/// Verifies two sequential calls in one caller preserve a coherent Phase 4 stack contract.
fn compiles_phase4_sequential_call_regression_fixture() {
    let output = compile_source(
        "pic16f628a",
        "sequential-calls.c",
        "\
#include <pic16/pic16f628a.h>
int add2(int a, int b) {
    return a + b;
}
int top_sum(void) {
    int x;
    int y;
    x = add2(1, 2);
    y = add2(3, 4);
    return x + y;
}
void main(void) {
    int total = top_sum();
    TRISB = 0x00;
    PORTB = total;
}
",
    );

    let asm = read_artifact(&output, "asm");
    assert_hex_is_programmable(&output);
    assert_phase4_stack_metadata(&output);
    assert_eq!(count_occurrences(&asm, "call fn_add2"), 2);
    assert!(asm.contains("call fn_top_sum"));
    assert!(asm.contains("frame args=4 saved_fp=4"));
}

#[test]
/// Verifies nested call chains emit consistent stack metadata and call lowering.
fn compiles_phase4_nested_call_regression_fixture() {
    let output = compile_source(
        "pic16f628a",
        "nested-calls.c",
        "\
#include <pic16/pic16f628a.h>
int f(int x) { return x + 1; }
int g(int y) { return f(y) + 2; }
int h(int z) { return g(z) + 3; }
void main(void) {
    int total = h(5);
    TRISB = 0x00;
    PORTB = total;
}
",
    );

    let asm = read_artifact(&output, "asm");
    assert_hex_is_programmable(&output);
    assert_phase4_stack_metadata(&output);
    assert!(asm.contains("call fn_f"));
    assert!(asm.contains("call fn_g"));
    assert!(asm.contains("call fn_h"));
}

#[test]
/// Verifies temps survive nested calls when one subexpression is lowered before a call.
fn compiles_phase4_temp_liveness_nested_call_fixture() {
    let output = compile_source(
        "pic16f628a",
        "temp-nested.c",
        "\
#include <pic16/pic16f628a.h>
int inc(int x) {
    return x + 1;
}
int combine(int a, int b, int c, int d) {
    return (a + b) + inc(c + d);
}
void main(void) {
    int total = combine(1, 2, 3, 4);
    TRISB = 0x00;
    PORTB = total;
}
",
    );

    let asm = read_artifact(&output, "asm");
    assert_hex_is_programmable(&output);
    assert_phase4_stack_metadata(&output);
    assert!(asm.contains("call fn_inc"));
    assert!(asm.contains("call fn_combine"));
}

#[test]
/// Verifies sibling call expressions keep caller temps live across two independent calls.
fn compiles_phase4_temp_liveness_sibling_calls_fixture() {
    let output = compile_source(
        "pic16f628a",
        "temp-siblings.c",
        "\
#include <pic16/pic16f628a.h>
int f(int x) {
    return x + 1;
}
void main(void) {
    int a;
    int b;
    int total;
    a = 10;
    b = 20;
    total = f(a + b) + f(a - b);
    TRISB = 0x00;
    PORTB = total;
}
",
    );

    let asm = read_artifact(&output, "asm");
    assert_hex_is_programmable(&output);
    assert_phase4_stack_metadata(&output);
    assert_eq!(count_occurrences(&asm, "call fn_f"), 2);
}

#[test]
/// Verifies 16-bit equality and inequality lowering through a temporary fixture.
fn compiles_16bit_equality_fixture() {
    let output = compile_source(
        "pic16f628a",
        "eq16.c",
        "\
#include <pic16/pic16f628a.h>
unsigned int mix16(unsigned int lhs, unsigned int rhs) {
    if (lhs == rhs) {
        return lhs + 1;
    }
    if (lhs != rhs) {
        return rhs - lhs;
    }
    return 0;
}
void main(void) {
    unsigned int value = mix16(3, 7);
    TRISB = 0x00;
    if (value != 0) {
        PORTB = 0x11;
    }
}
",
    );

    let ir = read_artifact(&output, "ir");
    let asm = read_artifact(&output, "asm");
    assert_hex_is_programmable(&output);
    assert!(ir.contains("Equal"));
    assert!(ir.contains("NotEqual"));
    assert!(asm.contains("fn_mix16"));
    assert!(asm.contains("call fn_mix16"));
    assert!(asm.contains("subwf"));
}

#[test]
/// Verifies local arrays, `sizeof`, and pointer traversal work in one integration fixture.
fn compiles_phase3_sizeof_and_local_array_fixture() {
    let output = compile_source(
        "pic16f877a",
        "sizeof-array.c",
        "\
#include <pic16/pic16f877a.h>
unsigned char result = 0;
/** Accumulates bytes from a caller-provided span. */
unsigned char accumulate(unsigned char *ptr, unsigned int len) {
    unsigned int i = 0;
    unsigned char acc = 0;
    while (i < len) {
        acc = acc + ptr[i];
        i = i + 1;
    }
    return acc;
}
/** Exercises local arrays, `sizeof`, pointer indexing, and indirect loads. */
void main(void) {
    unsigned char local[4];
    unsigned char *cursor = local;
    TRISB = 0x00;
    ADCON1 = 0x06;
    local[0] = sizeof(char);
    local[1] = sizeof(unsigned int);
    local[2] = sizeof(cursor);
    local[3] = sizeof(local);
    result = accumulate(local, sizeof(local));
    PORTB = result;
}
",
    );

    let ir = read_artifact(&output, "ir");
    let asm = read_artifact(&output, "asm");
    assert_hex_is_programmable(&output);
    assert!(ir.contains("call f"));
    assert!(asm.contains("call fn_accumulate"));
    assert!(asm.contains("movf 0x00,w"));
}

#[test]
/// Verifies taking the address of a stack local and passing it across a call compiles.
fn compiles_phase4_address_of_local_fixture() {
    let output = compile_source(
        "pic16f877a",
        "addr-local.c",
        "\
#include <pic16/pic16f877a.h>
/** Loads one byte through a caller-provided pointer. */
unsigned char load_byte(unsigned char *ptr, unsigned int index, unsigned char fallback) {
    if (index != 0) {
        return ptr[index];
    }
    return fallback;
}
/** Exercises `&local`, pointer arguments, and stack-backed local scalars. */
void main(void) {
    unsigned char local = 0x21;
    unsigned char *ptr = &local;
    TRISB = 0x00;
    ADCON1 = 0x06;
    PORTB = load_byte(ptr, 0, local);
}
",
    );

    let asm = read_artifact(&output, "asm");
    assert_hex_is_programmable(&output);
    assert!(asm.contains("call fn_load_byte"));
}

#[test]
/// Verifies helper calls survive alongside nested function calls and temp lifetimes.
fn compiles_phase5_helper_nested_expression_fixture() {
    let output = compile_source(
        "pic16f628a",
        "helper-nested.c",
        "\
#include <pic16/pic16f628a.h>
unsigned int inc(unsigned int x) {
    return x + 1;
}
unsigned int combine(unsigned int a, unsigned int b, unsigned int c, unsigned int d) {
    return (a + b) + inc(c * d);
}
void main(void) {
    TRISB = 0x00;
    PORTB = combine(1, 2, 3, 4);
}
",
    );
    let asm = read_artifact(&output, "asm");

    assert_hex_is_programmable(&output);
    assert_phase4_stack_metadata(&output);
    assert!(asm.contains("call __rt_mul_u16"));
    assert!(asm.contains("call fn_inc"));
}

#[test]
/// Verifies helper calls coexist with pointer and local-array lowering from earlier phases.
fn compiles_phase5_pointer_array_helper_fixture() {
    let output = compile_source(
        "pic16f877a",
        "pointer-shift.c",
        "\
#include <pic16/pic16f877a.h>
unsigned int shift_first(unsigned int *ptr, unsigned char n) {
    return ptr[0] >> n;
}
void main(void) {
    unsigned int words[2];
    words[0] = 0x0123;
    words[1] = 0x0040;
    ADCON1 = 0x06;
    TRISB = 0x00;
    PORTB = shift_first(words, 3);
}
",
    );

    assert_hex_is_programmable(&output);
    assert_phase4_stack_metadata(&output);
    assert_phase5_helper_artifacts(&output, "__rt_shr_u16");
}

#[test]
/// Verifies unsigned power-of-two division lowers inline instead of calling a runtime helper.
fn phase7_avoids_helper_for_unsigned_power_of_two_division() {
    let output = compile_source(
        "pic16f877a",
        "div-pow2.c",
        "\
#include <pic16/pic16f877a.h>
unsigned int quarter(unsigned int value) {
    return value / 4;
}
void main(void) {
    ADCON1 = 0x06;
    TRISB = 0x00;
    PORTB = quarter(0x0040);
}
",
    );

    let asm = read_artifact(&output, "asm");
    let map = read_artifact(&output, "map");
    assert_hex_is_programmable(&output);
    assert!(!asm.contains("call __rt_div_u16"));
    assert!(!map.contains("__rt_div_u16"));
}

#[test]
/// Verifies unsigned power-of-two modulo lowers to a mask instead of calling a runtime helper.
fn phase7_avoids_helper_for_unsigned_power_of_two_modulo() {
    let output = compile_source(
        "pic16f877a",
        "mod-pow2.c",
        "\
#include <pic16/pic16f877a.h>
unsigned int mod8(unsigned int value) {
    return value % 8;
}
void main(void) {
    ADCON1 = 0x06;
    TRISB = 0x00;
    PORTB = mod8(0x0037);
}
",
    );

    let asm = read_artifact(&output, "asm");
    let map = read_artifact(&output, "map");
    assert_hex_is_programmable(&output);
    assert!(!asm.contains("call __rt_mod_u16"));
    assert!(!map.contains("__rt_mod_u16"));
    assert!(asm.contains("andlw 0x07"));
}

#[test]
/// Verifies O2 IR optimization and backend cleanup shrink a trivial constant-branch fixture.
fn phase7_o2_reduces_instruction_count_for_constant_branch_fixture() {
    let source = "\
#include <pic16/pic16f628a.h>
void main(void) {
    unsigned char value = 0;
    TRISB = 0x00;
    if (1) {
        value = 3;
    } else {
        value = 4;
    }
    PORTB = value;
}
";
    let o0 =
        compile_source_with_optimization("pic16f628a", "const-branch-o0.c", source, OptimizationLevel::O0);
    let o2 =
        compile_source_with_optimization("pic16f628a", "const-branch-o2.c", source, OptimizationLevel::O2);
    let o0_asm = read_artifact(&o0, "asm");
    let o2_asm = read_artifact(&o2, "asm");

    let count_instructions = |asm: &str| {
        asm.lines()
            .filter(|line| {
                let trimmed = line.trim_start();
                !trimmed.is_empty()
                    && !trimmed.starts_with(';')
                    && !trimmed.ends_with(':')
                    && !trimmed.starts_with("org ")
            })
            .count()
    };

    assert!(count_instructions(&o2_asm) < count_instructions(&o0_asm));
}

#[test]
/// Verifies division by constant zero is rejected before lowering.
fn reports_division_by_constant_zero() {
    let error = execute(CliOptions {
        command: CliCommand::Compile(CompileCommand {
            target: "pic16f628a".to_string(),
            input: {
                let input = temp_file("div-zero.c");
                fs::write(
                    &input,
                    "\
#include <pic16/pic16f628a.h>
unsigned int bad(unsigned int value) {
    return value / 0;
}
void main(void) {
    TRISB = 0x00;
    PORTB = bad(7);
}
",
                )
                .expect("fixture");
                input
            },
            output: temp_file("div-zero.hex"),
            include_dirs: vec![repo("include")],
            defines: BTreeMap::new(),
            optimization: OptimizationLevel::O0,
            artifacts: OutputArtifacts::default(),
            verbose: false,
            opt_report: false,
            warning_profile: WarningProfile::default(),
        }),
    })
    .expect_err("must fail");

    assert!(format!("{error}").contains("division by constant zero"));
}

#[test]
/// Verifies modulo by constant zero is rejected before lowering.
fn reports_modulo_by_constant_zero() {
    let error = execute(CliOptions {
        command: CliCommand::Compile(CompileCommand {
            target: "pic16f628a".to_string(),
            input: {
                let input = temp_file("mod-zero.c");
                fs::write(
                    &input,
                    "\
#include <pic16/pic16f628a.h>
unsigned int bad(unsigned int value) {
    return value % 0;
}
void main(void) {
    TRISB = 0x00;
    PORTB = bad(7);
}
",
                )
                .expect("fixture");
                input
            },
            output: temp_file("mod-zero.hex"),
            include_dirs: vec![repo("include")],
            defines: BTreeMap::new(),
            optimization: OptimizationLevel::O0,
            artifacts: OutputArtifacts::default(),
            verbose: false,
            opt_report: false,
            warning_profile: WarningProfile::default(),
        }),
    })
    .expect_err("must fail");

    assert!(format!("{error}").contains("modulo by constant zero"));
}

#[test]
/// Verifies constant shift counts wider than the operand are rejected explicitly.
fn reports_constant_shift_count_too_wide() {
    let error = execute(CliOptions {
        command: CliCommand::Compile(CompileCommand {
            target: "pic16f628a".to_string(),
            input: {
                let input = temp_file("shift-wide.c");
                fs::write(
                    &input,
                    "\
#include <pic16/pic16f628a.h>
unsigned char bad(unsigned char value) {
    return value << 8;
}
void main(void) {
    TRISB = 0x00;
    PORTB = bad(1);
}
",
                )
                .expect("fixture");
                input
            },
            output: temp_file("shift-wide.hex"),
            include_dirs: vec![repo("include")],
            defines: BTreeMap::new(),
            optimization: OptimizationLevel::O0,
            artifacts: OutputArtifacts::default(),
            verbose: false,
            opt_report: false,
            warning_profile: WarningProfile::default(),
        }),
    })
    .expect_err("must fail");

    assert!(format!("{error}").contains("constant shift count"));
}

#[test]
/// Verifies interrupt handlers must return `void`.
fn reports_interrupt_return_type_mismatch() {
    let error = compile_error(
        "pic16f628a",
        "isr-ret-type.c",
        "\
#include <pic16/pic16f628a.h>
int __interrupt isr(void) {
    return 1;
}
void main(void) {
    TRISB = 0x00;
}
",
    );

    assert!(error.contains("must return `void`"));
}

#[test]
/// Verifies interrupt handlers cannot take parameters.
fn reports_interrupt_parameter_mismatch() {
    let error = compile_error(
        "pic16f628a",
        "isr-params.c",
        "\
#include <pic16/pic16f628a.h>
void __interrupt isr(int value) {
}
void main(void) {
    TRISB = 0x00;
}
",
    );

    assert!(error.contains("cannot take parameters"));
}

#[test]
/// Verifies Phase 6 rejects multiple interrupt handlers in one program.
fn reports_multiple_interrupt_handlers() {
    let error = compile_error(
        "pic16f628a",
        "two-isr.c",
        "\
#include <pic16/pic16f628a.h>
void __interrupt isr1(void) {
    PORTB = PORTB ^ 0x01;
}
void __interrupt isr2(void) {
    PORTB = PORTB ^ 0x02;
}
void main(void) {
    TRISB = 0x00;
}
",
    );

    assert!(error.contains("only one interrupt handler"));
}

#[test]
/// Verifies Phase 6 rejects normal function calls inside ISRs.
fn reports_interrupt_function_calls() {
    let error = compile_error(
        "pic16f628a",
        "isr-call.c",
        "\
#include <pic16/pic16f628a.h>
void helper(void) {
    PORTB = 0x33;
}
void __interrupt isr(void) {
    helper();
}
void main(void) {
    TRISB = 0x00;
}
",
    );

    assert!(error.contains("cannot call `helper`"));
}

#[test]
/// Verifies Phase 6 rejects runtime-helper arithmetic inside ISRs.
fn reports_interrupt_runtime_helper_arithmetic() {
    let error = compile_error(
        "pic16f628a",
        "isr-helper.c",
        "\
#include <pic16/pic16f628a.h>
void __interrupt isr(void) {
    int a = 3;
    int b = 4;
    int c;
    c = a * b;
    PORTB = c;
}
void main(void) {
    TRISB = 0x00;
}
",
    );

    assert!(error.contains("runtime helper"));
}

#[test]
/// Verifies mixed-sign 16-bit comparisons are rejected instead of silently coerced.
fn reports_mixed_signedness_compare() {
    let input = temp_file("mixed-signedness.c");
    fs::write(
        &input,
        "\
#include <pic16/pic16f628a.h>
void main(void) {
    int signed_value = -1;
    unsigned int unsigned_value = 1;
    TRISB = 0x00;
    if (signed_value < unsigned_value) {
        PORTB = 0x01;
    }
}
",
    )
    .expect("fixture");

    let error = execute(CliOptions {
        command: CliCommand::Compile(CompileCommand {
            target: "pic16f628a".to_string(),
            input,
            output: temp_file("mixed-signedness.hex"),
            include_dirs: vec![repo("include")],
            defines: BTreeMap::new(),
            optimization: OptimizationLevel::O0,
            artifacts: OutputArtifacts::default(),
            verbose: false,
            opt_report: false,
            warning_profile: WarningProfile::default(),
        }),
    })
    .expect_err("must fail");

    assert!(format!("{error}").contains("mixed signedness"));
}

#[test]
/// Verifies direct recursion is rejected under the current non-reentrant Phase 4 model.
fn reports_unsupported_recursion() {
    let error = execute(CliOptions {
        command: CliCommand::Compile(CompileCommand {
            target: "pic16f628a".to_string(),
            input: {
                let input = temp_file("recursion.c");
                fs::write(
                    &input,
                    "\
#include <pic16/pic16f628a.h>
int loop_forever(int value) {
    return loop_forever(value);
}
void main(void) {
    TRISB = 0x00;
    PORTB = loop_forever(1);
}
",
                )
                .expect("fixture");
                input
            },
            output: temp_file("recursion.hex"),
            include_dirs: vec![repo("include")],
            defines: BTreeMap::new(),
            optimization: OptimizationLevel::O0,
            artifacts: OutputArtifacts::default(),
            verbose: false,
            opt_report: false,
            warning_profile: WarningProfile::default(),
        }),
    })
    .expect_err("must fail");

    assert!(format!("{error}").contains("recursive call cycle"));
}

#[test]
/// Verifies returning the address of a stack local is rejected clearly.
fn reports_returning_stack_local_address() {
    let error = execute(CliOptions {
        command: CliCommand::Compile(CompileCommand {
            target: "pic16f628a".to_string(),
            input: {
                let input = temp_file("return-local.c");
                fs::write(
                    &input,
                    "\
#include <pic16/pic16f628a.h>
unsigned char *bad_ptr(void) {
    unsigned char local[2];
    return local;
}
void main(void) {
    unsigned char *ptr = bad_ptr();
    TRISB = 0x00;
    PORTB = ptr[0];
}
",
                )
                .expect("fixture");
                input
            },
            output: temp_file("return-local.hex"),
            include_dirs: vec![repo("include")],
            defines: BTreeMap::new(),
            optimization: OptimizationLevel::O0,
            artifacts: OutputArtifacts::default(),
            verbose: false,
            opt_report: false,
            warning_profile: WarningProfile::default(),
        }),
    })
    .expect_err("must fail");

    assert!(format!("{error}").contains("stack local"));
}

#[test]
/// Verifies obvious local-pointer alias chains are rejected before lowering.
fn reports_returning_stack_local_pointer_alias() {
    let error = execute(CliOptions {
        command: CliCommand::Compile(CompileCommand {
            target: "pic16f628a".to_string(),
            input: {
                let input = temp_file("return-local-alias.c");
                fs::write(
                    &input,
                    "\
#include <pic16/pic16f628a.h>
unsigned int *bad_ptr(void) {
    unsigned int local;
    unsigned int *p;
    p = &local;
    return p;
}
void main(void) {
    unsigned int *ptr = bad_ptr();
    TRISB = 0x00;
    PORTB = 0x00;
}
",
                )
                .expect("fixture");
                input
            },
            output: temp_file("return-local-alias.hex"),
            include_dirs: vec![repo("include")],
            defines: BTreeMap::new(),
            optimization: OptimizationLevel::O0,
            artifacts: OutputArtifacts::default(),
            verbose: false,
            opt_report: false,
            warning_profile: WarningProfile::default(),
        }),
    })
    .expect_err("must fail");

    assert!(format!("{error}").contains("stack local"));
}

#[test]
/// Verifies statically oversized local allocations fail with a stack-capacity diagnostic.
fn reports_oversized_local_allocation() {
    let error = execute(CliOptions {
        command: CliCommand::Compile(CompileCommand {
            target: "pic16f628a".to_string(),
            input: {
                let input = temp_file("oversized-local.c");
                fs::write(
                    &input,
                    "\
#include <pic16/pic16f628a.h>
void main(void) {
    unsigned char local[80];
    TRISB = 0x00;
    PORTB = local[0];
}
",
                )
                .expect("fixture");
                input
            },
            output: temp_file("oversized-local.hex"),
            include_dirs: vec![repo("include")],
            defines: BTreeMap::new(),
            optimization: OptimizationLevel::O0,
            artifacts: OutputArtifacts::default(),
            verbose: false,
            opt_report: false,
            warning_profile: WarningProfile::default(),
        }),
    })
    .expect_err("must fail");

    assert!(format!("{error}").contains("software stack"));
}

#[test]
/// Verifies pointer-to-pointer globals, address initializers, and indirect stores compile.
fn compiles_phase12_pointer_to_pointer_globals_and_argument_store() {
    let output = compile_source(
        "pic16f628a",
        "phase12-ptrptr.c",
        "\
#include <pic16/pic16f628a.h>
unsigned char value;
unsigned char *p = &value;
unsigned char **pp = &p;
void store_value(unsigned char **slot, unsigned char next) {
    **slot = next;
}
void main(void) {
    TRISB = 0x00;
    store_value(pp, 3);
    PORTB = value;
}
",
    );

    let asm = read_artifact(&output, "asm");
    let map = read_artifact(&output, "map");
    assert_hex_is_programmable(&output);
    assert!(asm.contains("address of value + 0"));
    assert!(asm.contains("address of p + 0"));
    assert!(map.contains("p"));
    assert!(map.contains("pp"));
}

#[test]
/// Verifies function pointer declarators fail with an explicit Phase 3 diagnostic.
fn reports_unsupported_function_pointer() {
    let error = execute(CliOptions {
        command: CliCommand::Compile(CompileCommand {
            target: "pic16f628a".to_string(),
            input: {
                let input = temp_file("fnptr.c");
                fs::write(
                    &input,
                    "\
#include <pic16/pic16f628a.h>
void main(void) {
    int (*fp)(void);
    TRISB = 0x00;
}
",
                )
                .expect("fixture");
                input
            },
            output: temp_file("fnptr.hex"),
            include_dirs: vec![repo("include")],
            defines: BTreeMap::new(),
            optimization: OptimizationLevel::O0,
            artifacts: OutputArtifacts::default(),
            verbose: false,
            opt_report: false,
            warning_profile: WarningProfile::default(),
        }),
    })
    .expect_err("must fail");

    assert!(format!("{error}").contains("function pointer declarators"));
}

#[test]
/// Verifies multidimensional arrays are rejected before lowering.
fn reports_unsupported_multidimensional_array() {
    let error = execute(CliOptions {
        command: CliCommand::Compile(CompileCommand {
            target: "pic16f628a".to_string(),
            input: {
                let input = temp_file("multidim.c");
                fs::write(
                    &input,
                    "\
#include <pic16/pic16f628a.h>
unsigned char table[2][2];
void main(void) {
    TRISB = 0x00;
}
",
                )
                .expect("fixture");
                input
            },
            output: temp_file("multidim.hex"),
            include_dirs: vec![repo("include")],
            defines: BTreeMap::new(),
            optimization: OptimizationLevel::O0,
            artifacts: OutputArtifacts::default(),
            verbose: false,
            opt_report: false,
            warning_profile: WarningProfile::default(),
        }),
    })
    .expect_err("must fail");

    assert!(format!("{error}").contains("multidimensional arrays"));
}

#[test]
/// Verifies compatible pointer subtraction compiles and scales 2-byte elements correctly.
fn compiles_phase12_pointer_subtraction() {
    let output = compile_source(
        "pic16f628a",
        "phase12-ptrsub.c",
        "\
#include <pic16/pic16f628a.h>
unsigned int words[3];
void main(void) {
    unsigned int *lhs = &words[2];
    unsigned int *rhs = &words[0];
    int diff = lhs - rhs;
    TRISB = 0x00;
    if (diff > 1) {
        PORTB = 0x11;
    } else {
        PORTB = 0x22;
    }
}
",
    );

    let ir = read_artifact(&output, "ir");
    assert_hex_is_programmable(&output);
    assert!(ir.contains("ShiftRight"));
}

#[test]
/// Verifies typedef aliases support scalar/pointer declarations and function signatures.
fn compiles_phase8_typedef_scalar_pointer_and_signature() {
    let output = compile_source(
        "pic16f628a",
        "phase8-typedef.c",
        "\
#include <pic16/pic16f628a.h>
typedef unsigned char u8;
typedef u8 *u8ptr;
u8 load_first(u8ptr ptr) {
    return ptr[0];
}
void main(void) {
    u8 values[2] = {1, 2};
    u8ptr cursor = values;
    TRISB = 0x00;
    PORTB = load_first(cursor);
}
",
    );

    let asm = read_artifact(&output, "asm");
    assert_hex_is_programmable(&output);
    assert!(asm.contains("call fn_load_first"));
}

#[test]
/// Verifies duplicate typedef names are rejected with a clear diagnostic.
fn reports_phase8_duplicate_typedef() {
    let error = compile_error(
        "pic16f628a",
        "phase8-dup-typedef.c",
        "\
#include <pic16/pic16f628a.h>
typedef unsigned char u8;
typedef unsigned int u8;
void main(void) {
    TRISB = 0x00;
}
",
    );

    assert!(error.contains("duplicate typedef"));
}

#[test]
/// Verifies enum implicit/explicit values compile and are usable in expressions.
fn compiles_phase8_enum_values_and_expression_use() {
    let output = compile_source(
        "pic16f628a",
        "phase8-enum.c",
        "\
#include <pic16/pic16f628a.h>
enum Mode {
    MODE_OFF,
    MODE_ON,
    MODE_ERROR = 10
};
unsigned char encode(enum Mode mode) {
    if (mode == MODE_ERROR) {
        return MODE_ON;
    }
    return MODE_OFF;
}
void main(void) {
    TRISB = 0x00;
    PORTB = encode(MODE_ERROR);
}
",
    );

    let ir = read_artifact(&output, "ir");
    assert_hex_is_programmable(&output);
    assert!(ir.contains("10"));
}

#[test]
/// Verifies duplicate enumerator names are rejected.
fn reports_phase8_duplicate_enumerator() {
    let error = compile_error(
        "pic16f628a",
        "phase8-dup-enum.c",
        "\
#include <pic16/pic16f628a.h>
enum Mode {
    MODE_OFF,
    MODE_OFF
};
void main(void) {
    TRISB = 0x00;
}
",
    );

    assert!(error.contains("duplicate enumerator"));
}

#[test]
/// Verifies struct field access works for globals/locals and pointer `->` forms.
fn compiles_phase8_struct_fields_and_arrow() {
    let output = compile_source(
        "pic16f628a",
        "phase8-struct-arrow.c",
        "\
#include <pic16/pic16f628a.h>
struct Pair {
    unsigned char lo;
    unsigned int hi;
};
struct Pair global_pair;
unsigned char touch_pair(struct Pair *ptr) {
    ptr->lo = 3;
    ptr->hi = 0x1234;
    return ptr->lo;
}
void main(void) {
    struct Pair local = {1, 2};
    struct Pair *cursor = &local;
    TRISB = 0x00;
    global_pair.lo = touch_pair(cursor);
    PORTB = global_pair.lo;
}
",
    );

    let ir = read_artifact(&output, "ir");
    let asm = read_artifact(&output, "asm");
    assert_hex_is_programmable(&output);
    assert!(ir.contains("Add 1"));
    assert!(asm.contains("movwf 0x04"));
}

#[test]
/// Verifies whole-struct copy assignment compiles through byte-wise lowering.
fn compiles_phase11_whole_struct_copy_assignment() {
    let output = compile_source(
        "pic16f628a",
        "phase11-struct-copy.c",
        "\
#include <pic16/pic16f628a.h>
struct Pair {
    unsigned char x;
    unsigned char y;
};
void main(void) {
    struct Pair a = {1, 2};
    struct Pair b = {3, 4};
    a = b;
    TRISB = 0x00;
    PORTB = a.y;
}
",
    );

    let ir = read_artifact(&output, "ir");
    assert_hex_is_programmable(&output);
    assert!(count_occurrences(&ir, "*t") >= 2);
}

#[test]
/// Verifies array/struct positional initializers compile with zero-fill for missing values.
fn compiles_phase8_array_and_struct_initializers() {
    let output = compile_source(
        "pic16f628a",
        "phase8-inits.c",
        "\
#include <pic16/pic16f628a.h>
struct Point {
    unsigned char x;
    unsigned char y;
};
unsigned char values[3] = {1, 2};
struct Point point = {7};
void main(void) {
    TRISB = 0x00;
    PORTB = values[2] + point.y;
}
",
    );

    assert_hex_is_programmable(&output);
}

#[test]
/// Verifies too many aggregate initializer elements emit a diagnostic.
fn reports_phase8_too_many_initializer_elements() {
    let error = compile_error(
        "pic16f628a",
        "phase8-init-too-many.c",
        "\
#include <pic16/pic16f628a.h>
unsigned char values[2] = {1, 2, 3};
void main(void) {
    TRISB = 0x00;
}
",
    );

    assert!(error.contains("too many initializer elements"));
}

#[test]
/// Verifies struct and array designated initializers compile with zero-fill.
fn compiles_phase11_designated_initializers() {
    let output = compile_source(
        "pic16f628a",
        "phase11-designated-init.c",
        "\
#include <pic16/pic16f628a.h>
struct Point {
    unsigned char x;
    unsigned char y;
};
struct Point point = {.y = 2, .x = 1};
unsigned char table[4] = {[0] = 1, [3] = 9};
void main(void) {
    TRISB = 0x00;
    PORTB = point.y + table[3];
}
",
    );

    let asm = read_artifact(&output, "asm");
    assert_hex_is_programmable(&output);
    assert!(asm.contains("init point"));
    assert!(asm.contains("init table"));
}

#[test]
/// Verifies arrays inside structs support direct and pointer-based element access.
fn compiles_phase11_array_field_in_struct_access() {
    let output = compile_source(
        "pic16f628a",
        "phase11-struct-array-field.c",
        "\
#include <pic16/pic16f628a.h>
struct Packet {
    unsigned char bytes[2];
    unsigned char length;
};
void main(void) {
    struct Packet packet = {{1, 0}, 1};
    struct Packet *ptr = &packet;
    TRISB = 0x00;
    ptr->bytes[1] = ptr->length;
    PORTB = packet.bytes[1];
}
",
    );

    let ir = read_artifact(&output, "ir");
    assert_hex_is_programmable(&output);
    assert!(ir.contains("Add 2"));
}

#[test]
/// Verifies nested struct fields support composed offsets and pointer-member access.
fn compiles_phase11_nested_struct_field_access() {
    let output = compile_source(
        "pic16f628a",
        "phase11-nested-struct-field.c",
        "\
#include <pic16/pic16f628a.h>
struct Point {
    unsigned char x;
    unsigned char y;
};
struct Box {
    struct Point top_left;
    struct Point bottom_right;
};
struct Box box = {{1, 2}, {3, 4}};
void main(void) {
    struct Box *ptr = &box;
    TRISB = 0x00;
    PORTB = ptr->bottom_right.y;
}
",
    );

    let asm = read_artifact(&output, "asm");
    assert_hex_is_programmable(&output);
    assert!(asm.contains("init box"));
}

#[test]
/// Verifies explicit narrowing casts avoid implicit-conversion diagnostics under `-Werror`.
fn allows_phase8_explicit_narrowing_cast_under_werror() {
    let output = compile_source_with_profile(
        "pic16f628a",
        "phase8-explicit-cast.c",
        "\
#include <pic16/pic16f628a.h>
void main(void) {
    unsigned int wide = 300;
    unsigned char narrow = (unsigned char)wide;
    TRISB = 0x00;
    PORTB = narrow;
}
",
        strict_warnings(),
    )
    .unwrap_or_else(|error| panic!("unexpected diagnostics: {error}"));

    assert_hex_is_programmable(&output);
}

#[test]
/// Verifies signed/unsigned explicit casts compile through Phase 8 typing rules.
fn compiles_phase8_signed_unsigned_explicit_casts() {
    let output = compile_source(
        "pic16f628a",
        "phase8-signed-casts.c",
        "\
#include <pic16/pic16f628a.h>
void main(void) {
    int signed_value = -1;
    unsigned int widened = (unsigned int)signed_value;
    unsigned char low = (unsigned char)widened;
    TRISB = 0x00;
    PORTB = low;
}
",
    );

    assert_hex_is_programmable(&output);
}

#[test]
/// Verifies unsupported integer-to-pointer explicit casts diagnose non-zero constants.
fn reports_phase8_unsupported_nonzero_integer_to_pointer_cast() {
    let error = compile_error(
        "pic16f628a",
        "phase8-bad-ptr-cast.c",
        "\
#include <pic16/pic16f628a.h>
void main(void) {
    unsigned char *ptr = (unsigned char*)1;
    TRISB = 0x00;
    PORTB = ptr[0];
}
",
    );

    assert!(error.contains("integer zero"));
}

#[test]
/// Verifies representable integer constants can initialize unsigned bytes under `-Werror`.
fn allows_representable_constant_to_unsigned_char() {
    let output = compile_source_with_profile(
        "pic16f628a",
        "fit-u8.c",
        "\
#include <pic16/pic16f628a.h>
void main(void) {
    unsigned char i = 8;
    TRISB = 0x00;
    PORTB = i;
}
",
        strict_warnings(),
    )
    .unwrap_or_else(|error| panic!("unexpected diagnostics: {error}"));

    assert_hex_is_programmable(&output);
}

#[test]
/// Verifies representable constants assigned to volatile SFR bytes do not warn.
fn allows_representable_constant_to_volatile_unsigned_char() {
    let output = compile_source_with_profile(
        "pic16f628a",
        "fit-volatile-u8.c",
        "\
#include <pic16/pic16f628a.h>
void main(void) {
    TRISB = 0x00;
    PORTB = 0x01;
}
",
        strict_warnings(),
    )
    .unwrap_or_else(|error| panic!("unexpected diagnostics: {error}"));

    assert_hex_is_programmable(&output);
}

#[test]
/// Verifies out-of-range constants still trigger narrowing diagnostics for unsigned bytes.
fn rejects_out_of_range_constant_to_unsigned_char() {
    let error = compile_source_with_profile(
        "pic16f628a",
        "oor-u8.c",
        "\
#include <pic16/pic16f628a.h>
void main(void) {
    unsigned char x = 300;
    TRISB = 0x00;
    PORTB = x;
}
",
        strict_warnings(),
    )
    .expect_err("must fail");

    assert!(error.contains("conversion from `int` to `unsigned char` truncates"));
}

#[test]
/// Verifies out-of-range constants still trigger narrowing diagnostics for volatile SFR bytes.
fn rejects_out_of_range_constant_to_volatile_unsigned_char() {
    let error = compile_source_with_profile(
        "pic16f628a",
        "oor-volatile-u8.c",
        "\
#include <pic16/pic16f628a.h>
void main(void) {
    TRISB = 0x00;
    PORTB = 300;
}
",
        strict_warnings(),
    )
    .expect_err("must fail");

    assert!(error.contains("conversion from `int` to `volatile unsigned char` truncates"));
}

#[test]
/// Verifies non-constant narrowing conversions still fail under `-Werror`.
fn rejects_non_constant_int_to_unsigned_char_under_werror() {
    let error = compile_source_with_profile(
        "pic16f628a",
        "nonconst-narrow-u8.c",
        "\
#include <pic16/pic16f628a.h>
void main(void) {
    int x = 1234;
    unsigned char y = x;
    TRISB = 0x00;
    PORTB = y;
}
",
        strict_warnings(),
    )
    .expect_err("must fail");

    assert!(error.contains("conversion from `int` to `unsigned char` truncates"));
}

#[test]
/// Verifies the checked-in blink example compiles with `-Wall -Wextra -Werror`.
fn blink_compiles_under_strict_warnings() {
    let output = compile_example_with_profile(
        "pic16f628a",
        "examples/pic16f628a/blink.c",
        strict_warnings(),
    )
    .unwrap_or_else(|error| panic!("unexpected diagnostics: {error}"));

    assert_hex_is_programmable(&output);
}

fn assert_makefile_shape(path: &str) {
    let makefile = fs::read_to_string(repo(path)).expect("makefile");
    assert!(makefile.contains("$(PIC)"));
    assert!(makefile.contains("--target"));
    assert!(makefile.contains("-o $(OUT)"));
    assert!(!makefile.contains("cargo run"));
}

#[test]
/// Verifies the release binary is named `picc`.
fn release_binary_is_picc() {
    let path = picc_bin();
    let stem = path.file_stem().and_then(|name| name.to_str()).expect("bin stem");
    assert_eq!(stem, "picc");
}

#[test]
/// Verifies `picc --help` prints the CLI usage text.
fn cli_help_works() {
    let output = Command::new(picc_bin())
        .current_dir(repo("."))
        .arg("--help")
        .output()
        .expect("run picc --help");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("picc"));
    assert!(stdout.contains("Usage:"));
}

#[test]
/// Verifies `picc --version` prints the version string.
fn cli_version_works() {
    let output = Command::new(picc_bin())
        .current_dir(repo("."))
        .arg("--version")
        .output()
        .expect("run picc --version");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("picc "));
}

#[test]
/// Verifies the CLI emits HEX, map, and listing outputs.
fn cli_generates_hex_map_and_list_outputs() {
    let out_dir = temp_dir_path("cli-out");
    let out_hex = out_dir.join("blink.hex");
    let output = Command::new(picc_bin())
        .current_dir(repo("."))
        .args([
            "--target",
            "pic16f628a",
            "-Wall",
            "-Wextra",
            "-Werror",
            "-O2",
            "-I",
            "include",
            "--map",
            "--list-file",
        ])
        .arg("-o")
        .arg(&out_hex)
        .arg("examples/pic16f628a/blink.c")
        .output()
        .expect("run picc");

    if !output.status.success() {
        panic!(
            "picc failed: stdout={:?} stderr={:?}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    assert_hex_is_programmable(&out_hex);
    assert!(out_hex.with_extension("map").exists());
    assert!(out_hex.with_extension("lst").exists());
}

#[test]
/// Verifies `--opt-report` prints the Phase 7 optimization summary after a successful build.
fn cli_opt_report_works() {
    let out_dir = temp_dir_path("cli-opt-report");
    let out_hex = out_dir.join("blink.hex");
    let output = Command::new(picc_bin())
        .current_dir(repo("."))
        .args([
            "--target",
            "pic16f628a",
            "-Wall",
            "-Wextra",
            "-O2",
            "--opt-report",
            "-I",
            "include",
            "--emit-asm",
        ])
        .arg("-o")
        .arg(&out_hex)
        .arg("examples/pic16f628a/blink.c")
        .output()
        .expect("run picc with --opt-report");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Optimization report (O2)"));
    assert!(stdout.contains("IR constant propagation/folding"));
    assert!(stdout.contains("Backend peephole"));
    assert!(stdout.contains("Helper calls avoided"));
    assert_hex_is_programmable(&out_hex);
}

#[test]
/// Verifies example Makefiles use the installed `picc` CLI shape.
fn example_makefiles_use_picc_cli() {
    assert_makefile_shape("examples/pic16f628a/Makefile");
    assert_makefile_shape("examples/pic16f877a/Makefile");
    assert_makefile_shape("examples/Makefile.template");
}

#[test]
/// Verifies the README command shape still parses into the expected CLI options.
fn parses_cli_shape_requested_in_readme() {
    let options = CliOptions::parse(vec![
        "picc".to_string(),
        "--target".to_string(),
        "pic16f877a".to_string(),
        "-Wall".to_string(),
        "-Wextra".to_string(),
        "-Werror".to_string(),
        "-O2".to_string(),
        "-I".to_string(),
        "include".to_string(),
        "-o".to_string(),
        "build/main.hex".to_string(),
        "src/main.c".to_string(),
    ])
    .expect("parse cli");

    let CliCommand::Compile(command) = options.command else {
        panic!("expected compile command");
    };
    assert_eq!(command.target, "pic16f877a");
    assert_eq!(command.output, PathBuf::from("build/main.hex"));
}

#[test]
/// Verifies named structs and global positional initializers compile and emit program HEX.
fn compiles_phase8_struct_global_initializer() {
    let output = compile_source(
        "pic16f877a",
        "phase8-struct-global.c",
        "\
#include <pic16/pic16f877a.h>
struct Point { unsigned int x; unsigned int y; };
struct Point p = { 1000, 2000 };
void main(void) {
    TRISB = 0x00;
    PORTB = p.x & 0xFF;
}
",
    );

    assert_hex_is_programmable(&output);
    let map = read_artifact(&output, "map");
    assert!(map.contains("p"));
}

#[test]
/// Verifies typedef, enum constants, and explicit casts compile in Phase 8.
fn compiles_phase8_enum_typedef_casts() {
    let output = compile_source(
        "pic16f628a",
        "phase8-enum-typedef.c",
        "\
#include <pic16/pic16f628a.h>
typedef unsigned int uint;
enum Flags { A = 1, B, C = 8 };
uint f = (uint)B;
unsigned char *np = (unsigned char*)0;
void main(void) {
    TRISB = 0x00;
    PORTB = f & 0xFF;
}
",
    );

    assert_hex_is_programmable(&output);
    let map = read_artifact(&output, "map");
    assert!(map.contains("f"));
}

#[test]
/// Verifies checked-in Phase 8 examples compile cleanly through the `picc` CLI.
fn phase8_examples_compile_via_picc() {
    let examples = [
        ("pic16f628a", "examples/pic16f628a/typedef_enum.c"),
        ("pic16f628a", "examples/pic16f628a/struct_point.c"),
        ("pic16f628a", "examples/pic16f628a/array_initializer.c"),
        ("pic16f628a", "examples/pic16f628a/struct_initializer.c"),
        ("pic16f628a", "examples/pic16f628a/casts.c"),
    ];

    for (target, example) in examples {
        let output = compile_example_via_picc_cli(target, example);
        assert_hex_is_programmable(&output);
        assert!(output.with_extension("map").exists());
        assert!(output.with_extension("lst").exists());
    }
}

#[test]
/// Verifies unsigned-byte switches lower to a compare chain and emit valid artifacts.
fn compiles_phase9_switch_unsigned_char_compare_chain() {
    let output = compile_source(
        "pic16f628a",
        "phase9-switch-u8.c",
        "\
#include <pic16/pic16f628a.h>
void main(void) {
    unsigned char state = 1;
    TRISB = 0x00;
    switch (state) {
        case 0:
            PORTB = 0x00;
            break;
        case 1:
            PORTB = 0x11;
            break;
        default:
            PORTB = 0xFF;
            break;
    }
}
",
    );

    let asm = read_artifact(&output, "asm");
    let listing = read_artifact(&output, "lst");
    let map = read_artifact(&output, "map");
    assert_hex_is_programmable(&output);
    assert!(count_occurrences(&asm, "subwf") >= 2);
    assert!(count_occurrences(&asm, "goto fn_main_b") >= 3);
    assert!(listing.contains("subwf"));
    assert!(map.contains("fn_main"));
}

#[test]
/// Verifies signed-16-bit switches compile without a default label.
fn compiles_phase9_switch_int_without_default() {
    let output = compile_source(
        "pic16f877a",
        "phase9-switch-int.c",
        "\
#include <pic16/pic16f877a.h>
void main(void) {
    int value = -1;
    ADCON1 = 0x06;
    TRISB = 0x00;
    switch (value) {
        case -1:
            PORTB = 0x0F;
            break;
        case 2:
            PORTB = 0xF0;
            break;
    }
}
",
    );

    assert_hex_is_programmable(&output);
}

#[test]
/// Verifies enum-backed switches compile through the fixed 16-bit enum model.
fn compiles_phase9_switch_enum() {
    let output = compile_source(
        "pic16f877a",
        "phase9-switch-enum.c",
        "\
#include <pic16/pic16f877a.h>
enum State {
    STATE_IDLE,
    STATE_RUN,
    STATE_ERROR = 9
};
void main(void) {
    enum State state = STATE_RUN;
    ADCON1 = 0x06;
    TRISB = 0x00;
    switch (state) {
        case STATE_IDLE:
            PORTB = 0x00;
            break;
        case STATE_RUN:
            PORTB = 0x01;
            break;
        default:
            PORTB = STATE_ERROR;
            break;
    }
}
",
    );

    assert_hex_is_programmable(&output);
}

#[test]
/// Verifies case fallthrough lowers without forcing an intermediate break.
fn compiles_phase9_switch_fallthrough() {
    let output = compile_source(
        "pic16f877a",
        "phase9-switch-fallthrough.c",
        "\
#include <pic16/pic16f877a.h>
void main(void) {
    unsigned char x = 1;
    unsigned char y = 0;
    ADCON1 = 0x06;
    TRISB = 0x00;
    switch (x) {
        case 1:
            y = 10;
        case 2:
            y = y + 1;
            break;
        default:
            y = 0xFF;
            break;
    }
    PORTB = y;
}
",
    );

    let ir = read_artifact(&output, "ir");
    assert_hex_is_programmable(&output);
    assert!(ir.contains("switch.case") || count_occurrences(&ir, "Equal") >= 2);
}

#[test]
/// Verifies nested switches compile with independent break targets.
fn compiles_phase9_nested_switch() {
    let output = compile_source(
        "pic16f628a",
        "phase9-nested-switch.c",
        "\
#include <pic16/pic16f628a.h>
void main(void) {
    unsigned char outer = 1;
    unsigned char inner = 2;
    TRISB = 0x00;
    switch (outer) {
        case 1:
            switch (inner) {
                case 2:
                    PORTB = 0x22;
                    break;
                default:
                    PORTB = 0x33;
                    break;
            }
            break;
        default:
            PORTB = 0xFF;
            break;
    }
}
",
    );

    assert_hex_is_programmable(&output);
}

#[test]
/// Verifies switch statements compile inside loops and `break` exits only the switch.
fn compiles_phase9_switch_inside_loop_break_exits_switch_only() {
    let output = compile_source(
        "pic16f628a",
        "phase9-switch-in-loop.c",
        "\
#include <pic16/pic16f628a.h>
void main(void) {
    unsigned char i = 0;
    unsigned char acc = 0;
    TRISB = 0x00;
    while (i < 3) {
        switch (i) {
            case 1:
                acc = acc + 2;
                break;
            default:
                acc = acc + 1;
                break;
        }
        i = i + 1;
    }
    PORTB = acc;
}
",
    );

    let ir = read_artifact(&output, "ir");
    assert_hex_is_programmable(&output);
    assert!(ir.contains("while.head"));
}

#[test]
/// Verifies loops nested inside one switch body compile cleanly.
fn compiles_phase9_loop_inside_switch() {
    let output = compile_source(
        "pic16f877a",
        "phase9-loop-in-switch.c",
        "\
#include <pic16/pic16f877a.h>
void main(void) {
    unsigned char mode = 0;
    unsigned char i = 0;
    ADCON1 = 0x06;
    TRISB = 0x00;
    switch (mode) {
        case 0:
            while (i < 2) {
                PORTB = i;
                i = i + 1;
            }
            break;
        default:
            PORTB = 0xFF;
            break;
    }
}
",
    );

    assert_hex_is_programmable(&output);
}

#[test]
/// Verifies duplicate case labels are rejected clearly.
fn reports_phase9_duplicate_case_value() {
    let error = compile_error(
        "pic16f628a",
        "phase9-dup-case.c",
        "\
#include <pic16/pic16f628a.h>
void main(void) {
    unsigned char x = 0;
    switch (x) {
        case 1:
            break;
        case 1:
            break;
    }
}
",
    );

    assert!(error.contains("duplicate case value"));
}

#[test]
/// Verifies multiple default labels are rejected clearly.
fn reports_phase9_multiple_defaults() {
    let error = compile_error(
        "pic16f628a",
        "phase9-multi-default.c",
        "\
#include <pic16/pic16f628a.h>
void main(void) {
    unsigned char x = 0;
    switch (x) {
        default:
            break;
        default:
            break;
    }
}
",
    );

    assert!(error.contains("multiple `default`"));
}

#[test]
/// Verifies `case` outside a switch is rejected clearly.
fn reports_phase9_case_outside_switch() {
    let error = compile_error(
        "pic16f628a",
        "phase9-case-outside.c",
        "\
#include <pic16/pic16f628a.h>
void main(void) {
    case 1:
        TRISB = 0x00;
}
",
    );

    assert!(error.contains("`case` label outside switch"));
}

#[test]
/// Verifies `default` outside a switch is rejected clearly.
fn reports_phase9_default_outside_switch() {
    let error = compile_error(
        "pic16f628a",
        "phase9-default-outside.c",
        "\
#include <pic16/pic16f628a.h>
void main(void) {
    default:
        TRISB = 0x00;
}
",
    );

    assert!(error.contains("`default` label outside switch"));
}

#[test]
/// Verifies non-constant case labels are rejected clearly.
fn reports_phase9_nonconstant_case_label() {
    let error = compile_error(
        "pic16f628a",
        "phase9-nonconst-case.c",
        "\
#include <pic16/pic16f628a.h>
void main(void) {
    unsigned char x = 0;
    unsigned char y = 1;
    switch (x) {
        case y:
            break;
        default:
            break;
    }
}
",
    );

    assert!(error.contains("case label must be a constant expression"));
}

#[test]
/// Verifies out-of-range case labels are rejected for the chosen switch type.
fn reports_phase9_case_value_not_representable() {
    let error = compile_error(
        "pic16f628a",
        "phase9-case-range.c",
        "\
#include <pic16/pic16f628a.h>
void main(void) {
    unsigned char x = 0;
    switch (x) {
        case 300:
            break;
        default:
            break;
    }
}
",
    );

    assert!(error.contains("not representable in switch type"));
}

#[test]
/// Verifies switches reject unsupported non-integer controlling expressions.
fn reports_phase9_switch_on_unsupported_type() {
    let error = compile_error(
        "pic16f628a",
        "phase9-switch-ptr.c",
        "\
#include <pic16/pic16f628a.h>
void main(void) {
    unsigned char value = 0;
    unsigned char *ptr = &value;
    switch (ptr) {
        case 0:
            break;
    }
}
",
    );

    assert!(error.contains("switch expression must have integer or enum type"));
}

#[test]
/// Verifies checked-in Phase 9 examples compile cleanly through the `picc` CLI.
fn phase9_examples_compile_via_picc() {
    let examples = [
        ("pic16f628a", "examples/pic16f628a/switch_state.c"),
        ("pic16f877a", "examples/pic16f877a/switch_enum.c"),
        ("pic16f877a", "examples/pic16f877a/switch_fallthrough.c"),
    ];

    for (target, example) in examples {
        let output = compile_example_via_picc_cli(target, example);
        assert_hex_is_programmable(&output);
        assert!(output.with_extension("map").exists());
        assert!(output.with_extension("lst").exists());
    }
}

#[test]
/// Verifies char and unsigned-char arrays can be initialized from string literals.
fn compiles_phase10_string_initialized_char_arrays() {
    let output = compile_source(
        "pic16f628a",
        "phase10-string-arrays.c",
        "\
#include <pic16/pic16f628a.h>
char msg_exact[3] = \"OK\";
unsigned char msg_infer[] = \"OK\";
void main(void) {
    TRISB = 0x00;
    PORTB = (unsigned char)msg_exact[0] + msg_infer[1];
}
",
    );

    let asm = read_artifact(&output, "asm");
    let map = read_artifact(&output, "map");
    assert_hex_is_programmable(&output);
    assert!(asm.contains("init msg_exact"));
    assert!(asm.contains("init msg_infer"));
    assert!(asm.contains("(3 byte payload)"));
    assert!(asm.contains("movlw 0x4F"));
    assert!(asm.contains("movlw 0x4B"));
    assert!(map.contains("msg_exact"));
    assert!(map.contains("msg_infer"));
}

#[test]
/// Verifies const/global/static data show clear startup comments and map tags.
fn compiles_phase10_const_and_static_data_startup() {
    let output = compile_source(
        "pic16f877a",
        "phase10-const-static.c",
        "\
#include <pic16/pic16f877a.h>
const unsigned char table[] = {1, 2, 3, 4};
static unsigned char flags[4];
unsigned int word = 0x1234;
void main(void) {
    ADCON1 = 0x06;
    TRISB = 0x00;
    flags[0] = table[0];
    PORTB = flags[0] + (unsigned char)word;
}
",
    );

    let asm = read_artifact(&output, "asm");
    let map = read_artifact(&output, "map");
    let listing = read_artifact(&output, "lst");
    assert_hex_is_programmable(&output);
    assert!(asm.contains("static data initialization"));
    assert!(asm.contains("init table [const]"));
    assert!(asm.contains("zero flags [static]"));
    assert!(asm.contains("init word"));
    let low = asm.find("movlw 0x34").expect("little-endian low byte init");
    let high = asm[low + 1..]
        .find("movlw 0x12")
        .map(|offset| low + 1 + offset)
        .expect("little-endian high byte init");
    assert!(low < high);
    assert!(map.contains("table [const]"));
    assert!(map.contains("flags [static]"));
    assert!(map.contains("word"));
    assert!(listing.contains("init table [const]"));
}

#[test]
/// Verifies static local initializers move into startup data handling.
fn compiles_phase10_static_local_initializer() {
    let output = compile_source(
        "pic16f628a",
        "phase10-static-local.c",
        "\
#include <pic16/pic16f628a.h>
void main(void) {
    static unsigned char seen = 3;
    TRISB = 0x00;
    PORTB = seen;
}
",
    );

    let asm = read_artifact(&output, "asm");
    let map = read_artifact(&output, "map");
    assert_hex_is_programmable(&output);
    assert!(asm.contains("init seen [static local]"));
    assert!(map.contains("seen [static local]"));
}

#[test]
/// Verifies string initializers that cannot fit including the trailing null are rejected.
fn reports_phase10_string_initializer_too_large() {
    let error = compile_error(
        "pic16f628a",
        "phase10-string-too-large.c",
        "\
#include <pic16/pic16f628a.h>
char msg[2] = \"OK\";
void main(void) {
    TRISB = 0x00;
}
",
    );

    assert!(error.contains("string initializer is too large"));
}

#[test]
/// Verifies omitted array sizes can be inferred from brace initializers for static data.
fn compiles_phase10_unsized_const_table_initializer() {
    let output = compile_source(
        "pic16f877a",
        "phase10-unsized-table.c",
        "\
#include <pic16/pic16f877a.h>
const unsigned char table[] = {1, 2, 3, 4};
void main(void) {
    ADCON1 = 0x06;
    TRISB = 0x00;
    PORTB = table[3];
}
",
    );

    let asm = read_artifact(&output, "asm");
    let map = read_artifact(&output, "map");
    assert_hex_is_programmable(&output);
    assert!(asm.contains("init table [const] @"));
    assert!(asm.contains("(4 byte payload)"));
    assert!(map.contains("table [const]"));
}

#[test]
/// Verifies string literals still diagnose clearly when assigned to one scalar target.
fn reports_phase10_string_literal_unsupported_context() {
    let error = compile_error(
        "pic16f628a",
        "phase10-string-context.c",
        "\
#include <pic16/pic16f628a.h>
void main(void) {
    PORTB = \"OK\";
}
",
    );

    assert!(error.contains("string literal is incompatible"));
}

#[test]
/// Verifies writes to const objects are rejected directly.
fn reports_phase10_const_assignment_rejected() {
    let error = compile_error(
        "pic16f628a",
        "phase10-const-assign.c",
        "\
#include <pic16/pic16f628a.h>
const unsigned char value = 1;
void main(void) {
    value = 2;
}
",
    );

    assert!(error.contains("assignment to const object"));
}

#[test]
/// Verifies const-qualified struct objects make their fields read-only too.
fn reports_phase10_const_struct_field_write_rejected() {
    let error = compile_error(
        "pic16f628a",
        "phase10-const-struct-field.c",
        "\
#include <pic16/pic16f628a.h>
struct Point {
    unsigned char x;
    unsigned char y;
};
const struct Point point = {1, 2};
void main(void) {
    point.y = 3;
}
",
    );

    assert!(error.contains("assignment to const object"));
}

#[test]
/// Verifies the documented Phase 12 const-qualified pointer forms compile cleanly.
fn compiles_phase12_const_pointer_forms() {
    let output = compile_source(
        "pic16f628a",
        "phase10-const-pointer.c",
        "\
#include <pic16/pic16f628a.h>
unsigned char x = 1;
unsigned char y = 2;
const unsigned char *ptr = &x;
void main(void) {
    TRISB = 0x00;
    unsigned char * const p2 = &x;
    const unsigned char * const p3 = &y;
    ptr = p2;
    PORTB = *ptr + *p3;
}
",
    );

    assert_hex_is_programmable(&output);
}

#[test]
/// Verifies checked-in Phase 10 examples compile cleanly through the `picc` CLI.
fn phase10_examples_compile_via_picc() {
    let examples = [
        ("pic16f628a", "examples/pic16f628a/string_array.c"),
        ("pic16f877a", "examples/pic16f877a/static_table.c"),
        ("pic16f877a", "examples/pic16f877a/const_config.c"),
        ("pic16f877a", "examples/pic16f877a/global_init.c"),
    ];

    for (target, example) in examples {
        let output = compile_example_via_picc_cli(target, example);
        assert_hex_is_programmable(&output);
        assert!(output.with_extension("map").exists());
        assert!(output.with_extension("lst").exists());
    }
}

#[test]
/// Verifies nested aggregate initializers and string array fields compile for arrays of structs.
fn compiles_phase11_nested_aggregate_initializers() {
    let output = compile_source(
        "pic16f877a",
        "phase11-config-table.c",
        "\
#include <pic16/pic16f877a.h>
struct PinConfig {
    unsigned char port;
    unsigned char bit;
};
struct DeviceConfig {
    struct PinConfig led;
    unsigned char name[4];
};
struct DeviceConfig configs[2] = {
    {{1, 0}, \"LED\"},
    {.led = {2, 3}, .name = \"BTN\"}
};
void main(void) {
    ADCON1 = 0x06;
    TRISB = 0x00;
    PORTB = configs[1].led.bit + configs[0].name[0];
}
",
    );

    let asm = read_artifact(&output, "asm");
    let map = read_artifact(&output, "map");
    assert_hex_is_programmable(&output);
    assert!(asm.contains("init configs"));
    assert!(map.contains("configs"));
}

#[test]
/// Verifies duplicate designated struct fields diagnose clearly.
fn reports_phase11_duplicate_designated_field() {
    let error = compile_error(
        "pic16f628a",
        "phase11-dup-designated-field.c",
        "\
#include <pic16/pic16f628a.h>
struct Point {
    unsigned char x;
    unsigned char y;
};
struct Point point = {.x = 1, .x = 2};
void main(void) {
    TRISB = 0x00;
}
",
    );

    assert!(error.contains("duplicate initializer for field"));
}

#[test]
/// Verifies duplicate array designators diagnose clearly.
fn reports_phase11_duplicate_array_designator() {
    let error = compile_error(
        "pic16f628a",
        "phase11-dup-array-designator.c",
        "\
#include <pic16/pic16f628a.h>
unsigned char table[2] = {[1] = 2, [1] = 3};
void main(void) {
    TRISB = 0x00;
}
",
    );

    assert!(error.contains("duplicate array initializer"));
}

#[test]
/// Verifies array designators must stay within bounds.
fn reports_phase11_array_designator_out_of_range() {
    let error = compile_error(
        "pic16f628a",
        "phase11-array-designator-range.c",
        "\
#include <pic16/pic16f628a.h>
unsigned char table[2] = {[2] = 1};
void main(void) {
    TRISB = 0x00;
}
",
    );

    assert!(error.contains("out of range"));
}

#[test]
/// Verifies array designators require constant integer expressions.
fn reports_phase11_nonconstant_array_designator() {
    let error = compile_error(
        "pic16f628a",
        "phase11-array-designator-nonconst.c",
        "\
#include <pic16/pic16f628a.h>
unsigned char index = 1;
unsigned char table[2] = {[index] = 1};
void main(void) {
    TRISB = 0x00;
}
",
    );

    assert!(error.contains("array designator index must be a constant expression"));
}

#[test]
/// Verifies self-containing structs by value are rejected clearly.
fn reports_phase11_self_containing_struct_by_value() {
    let error = compile_error(
        "pic16f628a",
        "phase11-self-struct.c",
        "\
#include <pic16/pic16f628a.h>
struct Node {
    struct Node child;
};
void main(void) {
    TRISB = 0x00;
}
",
    );

    assert!(error.contains("cannot contain itself by value"));
}

#[test]
/// Verifies incompatible named-struct assignments are rejected.
fn reports_phase11_incompatible_struct_assignment() {
    let error = compile_error(
        "pic16f628a",
        "phase11-incompatible-struct-assign.c",
        "\
#include <pic16/pic16f628a.h>
struct A {
    unsigned char x;
};
struct B {
    unsigned char x;
};
void main(void) {
    struct A a = {1};
    struct B b = {2};
    a = b;
    TRISB = 0x00;
}
",
    );

    assert!(error.contains("incompatible struct type"));
}

#[test]
/// Verifies assignments to const struct objects remain rejected after struct-copy support.
fn reports_phase11_const_struct_assignment_rejected() {
    let error = compile_error(
        "pic16f628a",
        "phase11-const-struct-assign.c",
        "\
#include <pic16/pic16f628a.h>
struct Pair {
    unsigned char x;
    unsigned char y;
};
const struct Pair a = {1, 2};
struct Pair b = {3, 4};
void main(void) {
    a = b;
    TRISB = 0x00;
}
",
    );

    assert!(error.contains("assignment to const object"));
}

#[test]
/// Verifies whole-struct assignment stays rejected inside interrupt handlers.
fn reports_phase11_struct_copy_in_isr() {
    let error = compile_error(
        "pic16f877a",
        "phase11-isr-struct-copy.c",
        "\
#include <pic16/pic16f877a.h>
struct Pair {
    unsigned char x;
    unsigned char y;
};
void __interrupt isr(void) {
    struct Pair a = {1, 2};
    struct Pair b = {3, 4};
    a = b;
}
void main(void) {
    ADCON1 = 0x06;
    TRISB = 0x00;
}
",
    );

    assert!(error.contains("whole-struct assignment is not supported inside interrupt handlers"));
}

#[test]
/// Verifies checked-in Phase 11 examples compile cleanly through the `picc` CLI.
fn phase11_examples_compile_via_picc() {
    let examples = [
        ("pic16f628a", "examples/pic16f628a/struct_array_field.c"),
        ("pic16f877a", "examples/pic16f877a/nested_struct.c"),
        ("pic16f877a", "examples/pic16f877a/designated_init.c"),
        ("pic16f877a", "examples/pic16f877a/struct_copy.c"),
        ("pic16f877a", "examples/pic16f877a/config_table.c"),
    ];

    for (target, example) in examples {
        let output = compile_example_via_picc_cli(target, example);
        assert_hex_is_programmable(&output);
        assert!(output.with_extension("map").exists());
        assert!(output.with_extension("lst").exists());
    }
}

#[test]
/// Verifies string literals may initialize RAM-backed data pointers and emit static symbols.
fn compiles_phase12_string_literal_pointer_initializer() {
    let output = compile_source(
        "pic16f877a",
        "phase12-string-pointer.c",
        "\
#include <pic16/pic16f877a.h>
char *msg = \"OK\";
const char *banner = \"HI\";
void main(void) {
    ADCON1 = 0x06;
    TRISB = 0x00;
    PORTB = (unsigned char)msg[0] + (unsigned char)banner[1];
}
",
    );

    let asm = read_artifact(&output, "asm");
    let map = read_artifact(&output, "map");
    assert_hex_is_programmable(&output);
    assert!(asm.contains("address of __strlit0"));
    assert!(asm.contains("address of __strlit1"));
    assert!(count_occurrences(&map, "__strlit") >= 2);
    assert!(map.contains("string literal"));
}

#[test]
/// Verifies pointer relational comparisons compile for compatible data-space pointer types.
fn compiles_phase12_pointer_relational_compare() {
    let output = compile_source(
        "pic16f628a",
        "phase12-pointer-compare.c",
        "\
#include <pic16/pic16f628a.h>
unsigned char bytes[4];
void main(void) {
    unsigned char *lhs = &bytes[1];
    const unsigned char *rhs = &bytes[2];
    TRISB = 0x00;
    if (lhs < rhs) {
        PORTB = 0x01;
    } else {
        PORTB = 0x02;
    }
}
",
    );

    assert_hex_is_programmable(&output);
}

#[test]
/// Verifies writes through pointer-to-const are rejected.
fn reports_phase12_write_through_pointer_to_const() {
    let error = compile_error(
        "pic16f628a",
        "phase12-write-through-const-ptr.c",
        "\
#include <pic16/pic16f628a.h>
unsigned char value = 1;
void main(void) {
    const unsigned char *ptr = &value;
    *ptr = 2;
}
",
    );

    assert!(error.contains("assignment to const object"));
}

#[test]
/// Verifies const pointer objects cannot be rebound after initialization.
fn reports_phase12_const_pointer_reassignment() {
    let error = compile_error(
        "pic16f628a",
        "phase12-const-pointer-reassign.c",
        "\
#include <pic16/pic16f628a.h>
unsigned char a = 1;
unsigned char b = 2;
void main(void) {
    unsigned char * const ptr = &a;
    ptr = &b;
}
",
    );

    assert!(error.contains("assignment to const object"));
}

#[test]
/// Verifies implicit qualifier discard across pointers is rejected.
fn reports_phase12_qualifier_discard() {
    let error = compile_error(
        "pic16f628a",
        "phase12-qualifier-discard.c",
        "\
#include <pic16/pic16f628a.h>
unsigned char value = 1;
void main(void) {
    const unsigned char *src = &value;
    unsigned char *dst = 0;
    dst = src;
}
",
    );

    assert!(error.contains("discarding qualifiers"));
}

#[test]
/// Verifies incompatible pointer assignments diagnose clearly.
fn reports_phase12_incompatible_pointer_assignment() {
    let error = compile_error(
        "pic16f628a",
        "phase12-incompatible-pointer.c",
        "\
#include <pic16/pic16f628a.h>
void main(void) {
    unsigned char bytes[2];
    unsigned char *bp = bytes;
    unsigned int *wp = 0;
    wp = bp;
}
",
    );

    assert!(error.contains("cannot coerce `char*`") || error.contains("cannot coerce `unsigned char*`"));
}

#[test]
/// Verifies pointer relational comparisons reject incompatible pointer types.
fn reports_phase12_invalid_pointer_relational_comparison() {
    let error = compile_error(
        "pic16f628a",
        "phase12-invalid-pointer-compare.c",
        "\
#include <pic16/pic16f628a.h>
unsigned char bytes[2];
unsigned int words[2];
void main(void) {
    unsigned char *bp = bytes;
    unsigned int *wp = words;
    if (bp < wp) {
        PORTB = 1;
    }
}
",
    );

    assert!(error.contains("pointer relational comparison requires compatible pointer types"));
}

#[test]
/// Verifies pointer subtraction rejects unsupported element sizes clearly.
fn reports_phase12_invalid_pointer_subtraction() {
    let error = compile_error(
        "pic16f628a",
        "phase12-invalid-pointer-subtraction.c",
        "\
#include <pic16/pic16f628a.h>
struct Triple {
    unsigned char a;
    unsigned char b;
    unsigned char c;
};
void main(void) {
    struct Triple items[2];
    struct Triple *lhs = &items[1];
    struct Triple *rhs = &items[0];
    if ((lhs - rhs) != 0) {
        PORTB = 1;
    }
}
",
    );

    assert!(error.contains("pointer subtraction for element type"));
}

#[test]
/// Verifies checked-in Phase 12 examples compile cleanly through the `picc` CLI.
fn phase12_examples_compile_via_picc() {
    let examples = [
        ("pic16f628a", "examples/pic16f628a/pointer_to_pointer.c"),
        ("pic16f877a", "examples/pic16f877a/const_pointers.c"),
        ("pic16f877a", "examples/pic16f877a/pointer_compare.c"),
        ("pic16f877a", "examples/pic16f877a/pointer_subtract.c"),
        ("pic16f877a", "examples/pic16f877a/string_pointer.c"),
    ];

    for (target, example) in examples {
        let output = compile_example_via_picc_cli(target, example);
        assert_hex_is_programmable(&output);
        assert!(output.with_extension("map").exists());
        assert!(output.with_extension("lst").exists());
    }
}

#[test]
/// Verifies ROM byte tables emit as RETLW program-memory objects and remain readable via `__rom_read8`.
fn compiles_phase13_rom_table_and_read() {
    let output = compile_source(
        "pic16f628a",
        "phase13-rom-table.c",
        "\
#include <pic16/pic16f628a.h>
const __rom unsigned char table[] = {1, 2, 3, 4};
void main(void) {
    TRISB = 0x00;
    PORTB = __rom_read8(table, 2);
}
",
    );

    let asm = read_artifact(&output, "asm");
    let map = read_artifact(&output, "map");
    let listing = read_artifact(&output, "lst");
    let hex = read_hex_bytes(&output);
    let rom_addr = map_symbol_address(&map, "table [rom, const]").expect("rom symbol");

    assert_hex_is_programmable(&output);
    assert!(map.contains("ROM Symbols"));
    assert!(asm.contains("program-memory ROM tables"));
    assert!(asm.contains("retlw 0x01"));
    assert!(asm.contains("retlw 0x04"));
    assert!(listing.contains("retlw 0x03"));
    assert!(rom_addr > 0x0004);
    assert_eq!(hex.get(&(rom_addr * 2 + 2)).copied(), Some(0x01));
    assert_eq!(hex.get(&(rom_addr * 2 + 3)).copied(), Some(0x34));
    assert_eq!(hex.get(&(rom_addr * 2 + 8)).copied(), Some(0x04));
    assert_eq!(hex.get(&(rom_addr * 2 + 9)).copied(), Some(0x34));
}

#[test]
/// Verifies explicit ROM strings compile as ROM byte arrays and stay visible in the map.
fn compiles_phase13_rom_string_array() {
    let output = compile_source(
        "pic16f877a",
        "phase13-rom-string.c",
        "\
#include <pic16/pic16f877a.h>
const __rom char msg[] = \"OK\";
void main(void) {
    ADCON1 = 0x06;
    TRISB = 0x00;
    PORTB = __rom_read8(msg, 1);
}
",
    );

    let map = read_artifact(&output, "map");
    let listing = read_artifact(&output, "lst");
    assert_hex_is_programmable(&output);
    assert!(map.contains("msg [rom, const]"));
    assert!(listing.contains("retlw 0x4F"));
    assert!(listing.contains("retlw 0x4B"));
    assert!(listing.contains("retlw 0x00"));
}

#[test]
/// Verifies non-const ROM objects are rejected clearly.
fn reports_phase13_nonconst_rom_object() {
    let error = compile_error(
        "pic16f628a",
        "phase13-rom-nonconst.c",
        "\
#include <pic16/pic16f628a.h>
__rom unsigned char table[] = {1, 2, 3};
void main(void) {
    TRISB = 0x00;
}
",
    );

    assert!(error.contains("must be declared `const`"));
}

#[test]
/// Verifies local ROM declarations are rejected because Phase 13 keeps ROM objects at file scope.
fn reports_phase13_local_rom_object() {
    let error = compile_error(
        "pic16f628a",
        "phase13-local-rom.c",
        "\
#include <pic16/pic16f628a.h>
void main(void) {
    const __rom unsigned char table[] = {1, 2, 3};
    TRISB = 0x00;
}
",
    );

    assert!(error.contains("cannot use `__rom` storage"));
}

#[test]
/// Verifies direct ROM indexing stays rejected in favor of the explicit builtin.
fn reports_phase13_direct_rom_indexing() {
    let error = compile_error(
        "pic16f628a",
        "phase13-rom-indexing.c",
        "\
#include <pic16/pic16f628a.h>
const __rom unsigned char table[] = {1, 2, 3};
void main(void) {
    PORTB = table[0];
}
",
    );

    assert!(error.contains("direct indexing of program-memory arrays"));
}

#[test]
/// Verifies data-space pointers cannot bind directly to ROM objects.
fn reports_phase13_data_pointer_to_rom() {
    let error = compile_error(
        "pic16f628a",
        "phase13-rom-pointer-mix.c",
        "\
#include <pic16/pic16f628a.h>
const __rom unsigned char table[] = {1, 2, 3};
unsigned char *ptr = table;
void main(void) {
    TRISB = 0x00;
}
",
    );

    assert!(error.contains("program-memory arrays do not decay to data-space pointers") || error.contains("cannot coerce"));
}

#[test]
/// Verifies ROM reads remain forbidden inside interrupt handlers in this phase.
fn reports_phase13_rom_read_in_isr() {
    let error = compile_error(
        "pic16f877a",
        "phase13-rom-isr.c",
        "\
#include <pic16/pic16f877a.h>
const __rom unsigned char table[] = {1, 2, 3};
void __interrupt isr(void) {
    PORTB = __rom_read8(table, 1);
}
void main(void) {
    ADCON1 = 0x06;
    TRISB = 0x00;
}
",
    );

    assert!(error.contains("ROM reads are not supported inside interrupt handlers"));
}

#[test]
/// Verifies checked-in Phase 13 examples compile cleanly through the `picc` CLI.
fn phase13_examples_compile_via_picc() {
    let examples = [
        ("pic16f628a", "examples/pic16f628a/rom_table.c"),
        ("pic16f877a", "examples/pic16f877a/rom_string.c"),
        ("pic16f877a", "examples/pic16f877a/rom_lookup.c"),
    ];

    for (target, example) in examples {
        let output = compile_example_via_picc_cli(target, example);
        assert_hex_is_programmable(&output);
        assert!(output.with_extension("map").exists());
        assert!(output.with_extension("lst").exists());
    }
}
