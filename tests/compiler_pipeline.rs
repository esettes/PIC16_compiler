// SPDX-License-Identifier: GPL-3.0-or-later

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

fn pic16_sim_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_pic16-sim"))
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
            stack_check: false,
            stack_report: false,
            stack_report_file: None,
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
            stack_check: false,
            stack_report: false,
            stack_report_file: None,
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
            stack_check: false,
            stack_report: false,
            stack_report_file: None,
            warning_profile: WarningProfile::default(),
        }),
    })
    .expect_err("must fail");
    format!("{error}")
}

fn compile_error_with_extra_args(
    target: &str,
    name: &str,
    source: &str,
    extra_args: &[&str],
) -> String {
    let input = temp_file(name);
    fs::write(&input, source).expect("fixture");
    let output = temp_file("error.hex");
    let mut command = std::process::Command::new(env!("CARGO_BIN_EXE_picc"));
    command.args(["--target", target, "-I", "include"]);
    command.args(extra_args);
    let output = command
        .arg("-o")
        .arg(output)
        .arg(input)
        .output()
        .expect("run picc error");
    assert!(!output.status.success(), "compile should fail");
    format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
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
            stack_check: false,
            stack_report: false,
            stack_report_file: None,
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
    compile_example_via_picc_cli_with_extra_args(target, input, &[])
}

/// Compiles one checked-in example through `picc` with extra CLI args.
fn compile_example_via_picc_cli_with_extra_args(
    target: &str,
    input: &str,
    extra_args: &[&str],
) -> PathBuf {
    let out_dir = temp_dir_path("phase8-cli");
    fs::create_dir_all(&out_dir).expect("out dir");
    let stem = Path::new(input)
        .file_stem()
        .and_then(|name| name.to_str())
        .expect("example stem");
    let out_hex = out_dir.join(format!("{stem}.hex"));
    let mut command = Command::new(picc_bin());
    command.current_dir(repo("."));
    command.args([
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
    ]);
    command.args(extra_args);
    let output = command
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

fn parse_program_words(output: &str) -> usize {
    output
        .lines()
        .find_map(|line| line.strip_prefix("Program words: "))
        .and_then(|rest| rest.split('/').next())
        .and_then(|words| words.trim().parse::<usize>().ok())
        .expect("program words in size output")
}

fn parse_runtime_helper_actual_words(report: &str, helper: &str) -> usize {
    report
        .lines()
        .find_map(|line| {
            line.strip_prefix(&format!("{helper}: "))
                .and_then(|rest| {
                    rest.split_whitespace()
                        .find_map(|part| part.strip_prefix("actual="))
                })
                .and_then(|actual| actual.parse::<usize>().ok())
        })
        .unwrap_or_else(|| panic!("runtime helper {helper} actual word count in memory report"))
}

fn rendered_map_has_symbol(map: &str, symbol: &str) -> bool {
    map.lines()
        .any(|line| line.split_whitespace().nth(1) == Some(symbol))
}

fn compile_profile_size_report(
    profile: &str,
    input: &str,
    name: &str,
    extra_args: &[&str],
) -> (PathBuf, String, String) {
    let out_dir = temp_dir_path(name);
    fs::create_dir_all(&out_dir).expect("out dir");
    let out_hex = out_dir.join(format!("{profile}.hex"));
    let memory_report = out_dir.join(format!("{profile}.mem"));
    let mut command = Command::new(picc_bin());
    command.current_dir(repo("."));
    command.args([
        "--target",
        "pic16f877a",
        "-Wall",
        "-Wextra",
        "-O2",
        "-I",
        "include",
        "--runtime-profile",
        profile,
        "--size",
        "--memory-report",
        "--memory-report-file",
    ]);
    command.arg(&memory_report);
    command.args(["--map", "--list-file"]);
    command.args(extra_args);
    let output = command
        .arg("-o")
        .arg(&out_hex)
        .arg(input)
        .output()
        .expect("run picc profile report");

    if !output.status.success() {
        panic!(
            "picc profile {profile} failed for {input}: stdout={:?} stderr={:?}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    (
        out_hex,
        String::from_utf8_lossy(&output.stdout).into_owned(),
        fs::read_to_string(memory_report).expect("memory report"),
    )
}

fn compile_profile_source_size_report(
    profile: &str,
    name: &str,
    source: &str,
    extra_args: &[&str],
) -> (PathBuf, String, String) {
    let source_path = temp_file(&format!("{name}.c"));
    fs::write(&source_path, source).expect("fixture");
    compile_profile_size_report(
        profile,
        source_path.to_str().expect("utf8 temp source"),
        name,
        extra_args,
    )
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
/// Verifies the Phase 4 stack ABI handles 3+ arguments and nested calls.
fn compiles_phase4_stack_abi_example() {
    let output = compile_example("pic16f877a", "examples/pic16f628a/stack_abi.c");
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
    let o0 = compile_source_with_optimization(
        "pic16f628a",
        "const-branch-o0.c",
        source,
        OptimizationLevel::O0,
    );
    let o2 = compile_source_with_optimization(
        "pic16f628a",
        "const-branch-o2.c",
        source,
        OptimizationLevel::O2,
    );
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
            stack_check: false,
            stack_report: false,
            stack_report_file: None,
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
            stack_check: false,
            stack_report: false,
            stack_report_file: None,
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
            stack_check: false,
            stack_report: false,
            stack_report_file: None,
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
            stack_check: false,
            stack_report: false,
            stack_report_file: None,
            warning_profile: WarningProfile::default(),
        }),
    })
    .expect_err("must fail");

    assert!(format!("{error}").contains("mixed signedness"));
}

#[test]
/// Verifies direct recursion is rejected under the current Phase 18 stack policy.
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
            stack_check: false,
            stack_report: false,
            stack_report_file: None,
            warning_profile: WarningProfile::default(),
        }),
    })
    .expect_err("must fail");

    assert!(format!("{error}").contains("recursive call cycle"));
}

#[test]
/// Verifies mutual recursion is rejected even when one side starts as a prototype.
fn reports_phase18_mutual_recursion_cycle() {
    let error = compile_error(
        "pic16f877a",
        "phase18-mutual-recursion.c",
        "\
#include <pic16/pic16f877a.h>
unsigned char odd(unsigned char value);
unsigned char even(unsigned char value) {
    if (value == 0) {
        return 1;
    }
    return odd(value - 1);
}
unsigned char odd(unsigned char value) {
    if (value == 0) {
        return 0;
    }
    return even(value - 1);
}
void main(void) {
    ADCON1 = 0x06;
    TRISB = 0x00;
    PORTB = even(4);
}
",
    );

    assert!(error.contains("recursive call cycle"));
    assert!(error.contains("even -> odd -> even") || error.contains("odd -> even -> odd"));
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
            stack_check: false,
            stack_report: false,
            stack_report_file: None,
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
            stack_check: false,
            stack_report: false,
            stack_report_file: None,
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
            stack_check: false,
            stack_report: false,
            stack_report_file: None,
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
/// Verifies raw function-pointer declarators now compile and lower through the dispatcher path.
fn compiles_phase17_raw_function_pointer_variable() {
    let output = compile_source(
        "pic16f628a",
        "phase17-raw-fnptr.c",
        "\
#include <pic16/pic16f628a.h>
void off(void) {
    PORTB = 0;
}
void on(void) {
    PORTB = 1;
}
void (*handler)(void);
void main(void) {
    TRISB = 0x00;
    handler = on;
    handler();
    handler = off;
    handler();
}
",
    );

    let asm = read_artifact(&output, "asm");
    let map = read_artifact(&output, "map");
    assert_hex_is_programmable(&output);
    assert!(asm.contains("__fp_dispatch_"));
    assert!(map.contains("__fp_dispatch_"));
}

#[test]
/// Verifies one typed function pointer may carry an 8-bit argument and return value.
fn compiles_phase17_function_pointer_u8_signature() {
    let output = compile_source(
        "pic16f877a",
        "phase17-fnptr-u8.c",
        "\
#include <pic16/pic16f877a.h>
typedef unsigned char (*Transform)(unsigned char);
unsigned char plus_two(unsigned char value) {
    return value + 2;
}
void main(void) {
    Transform fn;
    unsigned char result;
    ADCON1 = 0x06;
    TRISB = 0x00;
    fn = plus_two;
    result = fn(3);
    PORTB = result;
}
",
    );

    assert_hex_is_programmable(&output);
}

#[test]
/// Verifies one typed function pointer may carry a 16-bit argument and return value.
fn compiles_phase17_function_pointer_u16_signature() {
    let output = compile_source(
        "pic16f877a",
        "phase17-fnptr-u16.c",
        "\
#include <pic16/pic16f877a.h>
typedef unsigned int (*WordTransform)(unsigned int);
unsigned int plus_word(unsigned int value) {
    return value + 5;
}
void main(void) {
    WordTransform fn;
    unsigned int result;
    ADCON1 = 0x06;
    TRISB = 0x00;
    fn = plus_word;
    result = fn(1000);
    PORTB = (unsigned char)result;
}
",
    );

    assert_hex_is_programmable(&output);
}

#[test]
/// Verifies arrays of function pointers dispatch through indexed loads plus one generated dispatcher.
fn compiles_phase17_function_pointer_table_dispatch() {
    let output = compile_source(
        "pic16f877a",
        "phase17-fnptr-table.c",
        "\
#include <pic16/pic16f877a.h>
typedef void (*Handler)(void);
void off(void) {
    PORTB = 0;
}
void on(void) {
    PORTB = 1;
}
Handler table[2] = {off, on};
void main(void) {
    unsigned char index;
    ADCON1 = 0x06;
    TRISB = 0x00;
    index = 1;
    table[index]();
}
",
    );

    let asm = read_artifact(&output, "asm");
    let map = read_artifact(&output, "map");
    assert_hex_is_programmable(&output);
    assert!(asm.contains("__fp_dispatch_"));
    assert!(asm.contains("dispatch id 1"));
    assert!(map.contains("__fp_dispatch_"));
    assert!(map.contains("__id_1_"));
}

#[test]
/// Verifies function-pointer fields inside structs remain callable after aggregate field access.
fn compiles_phase17_function_pointer_struct_field_dispatch() {
    let output = compile_source(
        "pic16f877a",
        "phase17-fnptr-struct.c",
        "\
#include <pic16/pic16f877a.h>
typedef void (*Handler)(void);
struct Device {
    Handler handler;
};
void on(void) {
    PORTB = 1;
}
void main(void) {
    struct Device device;
    ADCON1 = 0x06;
    TRISB = 0x00;
    device.handler = on;
    device.handler();
}
",
    );

    assert_hex_is_programmable(&output);
}

#[test]
/// Verifies dispatcher code is emitted only when one function pointer is actually used.
fn phase17_dispatcher_generated_only_when_needed() {
    let direct_only = compile_source(
        "pic16f628a",
        "phase17-direct-only.c",
        "\
#include <pic16/pic16f628a.h>
void on(void) {
    PORTB = 1;
}
void main(void) {
    TRISB = 0x00;
    on();
}
",
    );
    let indirect = compile_source(
        "pic16f628a",
        "phase17-indirect.c",
        "\
#include <pic16/pic16f628a.h>
typedef void (*Handler)(void);
void on(void) {
    PORTB = 1;
}
Handler handler = on;
void main(void) {
    TRISB = 0x00;
    handler();
}
",
    );

    assert!(!read_artifact(&direct_only, "asm").contains("__fp_dispatch_"));
    assert!(read_artifact(&indirect, "asm").contains("__fp_dispatch_"));
}

#[test]
/// Verifies incompatible signatures are rejected on function-pointer assignment.
fn reports_phase17_incompatible_function_pointer_assignment() {
    let error = compile_error(
        "pic16f877a",
        "phase17-incompatible-fnptr.c",
        "\
typedef void (*VoidHandler)(void);
typedef unsigned char (*ByteHandler)(unsigned char);
void off(void) {
}
ByteHandler handler;
void main(void) {
    handler = off;
}
",
    );

    assert!(error.contains("incompatible function pointer conversion"));
}

#[test]
/// Verifies relational comparison of function pointers is rejected clearly.
fn reports_phase17_function_pointer_relational_compare() {
    let error = compile_error(
        "pic16f877a",
        "phase17-fnptr-rel.c",
        "\
typedef void (*Handler)(void);
void off(void) {
}
void on(void) {
}
void main(void) {
    Handler a = off;
    Handler b = on;
    if (a < b) {
        PORTB = 1;
    }
}
",
    );

    assert!(error.contains("relational comparison of function pointers"));
}

#[test]
/// Verifies arithmetic on function pointers is rejected clearly.
fn reports_phase17_function_pointer_arithmetic() {
    let error = compile_error(
        "pic16f877a",
        "phase17-fnptr-arith.c",
        "\
typedef void (*Handler)(void);
void off(void) {
}
void main(void) {
    Handler h = off;
    h = h + 1;
}
",
    );

    assert!(error.contains("function-pointer arithmetic"));
}

#[test]
/// Verifies data pointers and function pointers do not silently mix.
fn reports_phase17_data_pointer_function_pointer_mixing() {
    let error = compile_error(
        "pic16f877a",
        "phase17-fnptr-mix.c",
        "\
typedef void (*Handler)(void);
unsigned char value;
unsigned char *ptr = &value;
Handler handler;
void main(void) {
    handler = ptr;
}
",
    );

    assert!(error.contains("cannot convert data pointer"));
}

#[test]
/// Verifies function-pointer calls remain forbidden inside interrupt handlers.
fn reports_phase17_function_pointer_call_in_isr() {
    let error = compile_error(
        "pic16f877a",
        "phase17-fnptr-isr.c",
        "\
#include <pic16/pic16f877a.h>
typedef void (*Handler)(void);
void on(void) {
    PORTB = 1;
}
Handler handler = on;
void __interrupt isr(void) {
    handler();
}
void main(void) {
    ADCON1 = 0x06;
    TRISB = 0x00;
}
",
    );

    assert!(error.contains("function-pointer calls are not supported inside interrupt handlers"));
}

#[test]
/// Verifies checked-in Phase 17 examples compile cleanly through the `picc` CLI.
fn phase17_examples_compile_via_picc() {
    let examples = [
        ("pic16f628a", "examples/pic16f628a/function_pointer_basic.c"),
        ("pic16f877a", "examples/pic16f877a/function_pointer_table.c"),
        (
            "pic16f877a",
            "examples/pic16f877a/function_pointer_struct.c",
        ),
        ("pic16f877a", "examples/pic16f877a/state_dispatch_fp.c"),
    ];

    for (target, example) in examples {
        let output = compile_example_via_picc_cli(target, example);
        assert_hex_is_programmable(&output);
        assert!(output.with_extension("map").exists());
        assert!(output.with_extension("lst").exists());
    }
}

#[test]
/// Verifies fixed multidimensional arrays compile and use row-major indexing.
fn compiles_phase16_multidimensional_array_basic() {
    let output = compile_source(
        "pic16f877a",
        "phase16-matrix-basic.c",
        "\
#include <pic16/pic16f877a.h>
unsigned char matrix[2][3];
void main(void) {
    unsigned char i = 1;
    unsigned char j = 2;
    matrix[0][0] = 1;
    matrix[i][j] = 9;
    TRISB = 0x00;
    PORTB = matrix[1][2];
}
",
    );

    let ir = read_artifact(&output, "ir");
    assert_hex_is_programmable(&output);
    assert!(ir.contains("Multiply") || ir.contains("Add"));
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
        "pic16f877a",
        "phase11-struct-copy.c",
        "\
#include <pic16/pic16f877a.h>
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
/// Verifies Phase 21 parses 32-bit integer declarations and literal suffixes.
fn compiles_phase21_long_declarations_and_literals() {
    let output = compile_source(
        "pic16f628a",
        "phase21-long-types.c",
        "\
unsigned long global = 100000UL;
long signed_global = -200000L;
unsigned long result;
void main(void) {
    long local;
    local = signed_global;
    result = global + (unsigned long)local + 0x12345678UL;
}
",
    );

    assert_hex_is_programmable(&output);
}

#[test]
/// Verifies implicit narrowing from a 32-bit value is diagnosed under `-Werror`.
fn rejects_phase21_implicit_long_narrowing_under_werror() {
    let error = compile_source_with_profile(
        "pic16f628a",
        "phase21-long-narrow.c",
        "\
unsigned long source;
unsigned int result;
void main(void) {
    result = source;
}
",
        strict_warnings(),
    )
    .expect_err("must fail");

    assert!(error.contains("conversion from `unsigned long` to `unsigned int` truncates"));
}

#[test]
/// Verifies explicit narrowing casts from 32-bit values are accepted.
fn allows_phase21_explicit_long_narrowing_cast() {
    let output = compile_source_with_profile(
        "pic16f628a",
        "phase21-long-explicit-narrow.c",
        "\
unsigned long source;
unsigned int result;
void main(void) {
    result = (unsigned int)source;
}
",
        strict_warnings(),
    )
    .unwrap_or_else(|error| panic!("unexpected diagnostics: {error}"));

    assert_hex_is_programmable(&output);
}

#[test]
/// Verifies integer literals above the supported 32-bit range are rejected.
fn rejects_phase21_literal_too_large_for_u32() {
    let error = compile_error(
        "pic16f628a",
        "phase21-literal-too-large.c",
        "\
unsigned long result;
void main(void) {
    result = 4294967296UL;
}
",
    );

    assert!(error.contains("integer literal does not fit unsigned long"));
}

#[test]
/// Verifies deferred 32-bit ROM objects produce a clear diagnostic.
fn rejects_phase21_rom_long_objects() {
    let error = compile_error(
        "pic16f628a",
        "phase21-rom-long.c",
        "\
const __rom unsigned long table[] = { 1UL, 2UL };
unsigned long result;
void main(void) {
    result = 0UL;
}
",
    );

    assert!(error.contains("unsupported ROM element type"));
}

#[test]
/// Verifies helper-backed 32-bit operations stay rejected inside interrupt handlers.
fn rejects_phase21_long_helper_inside_isr() {
    let error = compile_error(
        "pic16f628a",
        "phase21-isr-long-helper.c",
        "\
unsigned long a;
unsigned long b;
unsigned long result;
void __interrupt isr(void) {
    result = a * b;
}
void main(void) {
}
",
    );

    assert!(error.contains("cannot use `Multiply` when it would lower through a runtime helper"));
}

#[test]
/// Verifies checked-in Phase 21 long examples compile cleanly.
fn phase21_examples_compile_via_picc() {
    let examples = [
        ("pic16f628a", "examples/pic16f628a/long_basic.c"),
        ("pic16f877a", "examples/pic16f877a/long_arithmetic.c"),
        ("pic16f877a", "examples/pic16f877a/long_struct.c"),
        ("pic16f877a", "examples/pic16f877a/long_array.c"),
        ("pic16f877a", "examples/pic16f877a/long_counter.c"),
    ];

    for (target, path) in examples {
        let output = compile_example(target, path);
        assert_hex_is_programmable(&output);
    }
}

#[test]
/// Verifies Phase 22 parses fixed-point declarations, casts, and raw constructors.
fn compiles_phase22_fixed_declarations_and_raw_constructors() {
    let output = compile_source(
        "pic16f628a",
        "phase22-fixed-types.c",
        "\
__fixed8_8 q = __q8_8(384);
__ufixed8_8 uq = __uq8_8(512);
__fixed16_16 q32 = __q16_16(0x00018000);
__ufixed16_16 uq32 = __uq16_16(0x00020000);
__fixed8_8 result;
void main(void) {
    int whole;
    whole = (int)q;
    result = (__fixed8_8)whole + (__fixed8_8)uq;
}
",
    );

    assert_hex_is_programmable(&output);
}

#[test]
/// Verifies implicit fixed precision narrowing is diagnosed under `-Werror`.
fn rejects_phase22_implicit_fixed_narrowing_under_werror() {
    let error = compile_source_with_profile(
        "pic16f628a",
        "phase22-fixed-narrow.c",
        "\
__fixed16_16 source;
__fixed8_8 result;
void main(void) {
    result = source;
}
",
        strict_warnings(),
    )
    .expect_err("must fail");

    assert!(error.contains("narrows fixed-point precision"));
}

#[test]
/// Verifies bitwise operators on fixed-point values require explicit raw casts.
fn rejects_phase22_fixed_bitwise_operations() {
    let error = compile_error(
        "pic16f628a",
        "phase22-fixed-bitwise.c",
        "\
__fixed8_8 a;
__fixed8_8 b;
__fixed8_8 result;
void main(void) {
    result = a & b;
}
",
    );

    assert!(error.contains("is not supported for fixed-point operands"));
}

#[test]
/// Verifies Phase 24 lowers dynamic Q16.16 multiply/divide through runtime helpers.
fn compiles_phase24_dynamic_q16_16_helpers() {
    let mul_output = compile_source(
        "pic16f877a",
        "phase24-q16-dynamic-mul.c",
        "\
__fixed16_16 a;
__fixed16_16 b;
__fixed16_16 product;
void main(void) {
    a = 1.5q16_16;
    b = 2.0q16_16;
    product = a * b;
}
",
    );

    assert_hex_is_programmable(&mul_output);
    let mul_map = read_artifact(&mul_output, "map");
    let mul_lst = read_artifact(&mul_output, "lst");
    assert!(mul_map.contains("__rt_mul_q16_16"));
    assert!(mul_lst.contains("__rt_mul_q16_16"));

    let div_output = compile_source(
        "pic16f877a",
        "phase24-q16-dynamic-div.c",
        "\
__fixed16_16 numerator;
__fixed16_16 denominator;
__fixed16_16 quotient;
void main(void) {
    numerator = 3.0q16_16;
    denominator = 2.0q16_16;
    quotient = numerator / denominator;
}
",
    );

    assert_hex_is_programmable(&div_output);
    let div_map = read_artifact(&div_output, "map");
    let div_lst = read_artifact(&div_output, "lst");
    assert!(div_map.contains("__rt_div_q16_16"));
    assert!(div_lst.contains("__rt_div_q16_16"));
}

#[test]
/// Verifies stack reports account for Phase 24 Q16.16 helper calls.
fn phase24_stack_report_accounts_for_q16_helpers() {
    let input = temp_file("phase24-q16-stack.c");
    let report = temp_file("phase24-q16.stack");
    fs::write(
        &input,
        "\
__fixed16_16 a;
__fixed16_16 b;
__fixed16_16 result;
void main(void) {
    a = 1.5q16_16;
    b = 2.0q16_16;
    result = a * b;
}
",
    )
    .expect("fixture");
    execute(CliOptions {
        command: CliCommand::Compile(CompileCommand {
            target: "pic16f877a".to_string(),
            input,
            output: temp_file("phase24-q16-stack.hex"),
            include_dirs: vec![repo("include")],
            defines: BTreeMap::new(),
            optimization: OptimizationLevel::O2,
            artifacts: OutputArtifacts::default(),
            verbose: false,
            opt_report: false,
            stack_check: true,
            stack_report: false,
            stack_report_file: Some(report.clone()),
            warning_profile: WarningProfile::default(),
        }),
    })
    .expect("compile with stack report file");

    let text = fs::read_to_string(report).expect("stack report");
    assert!(text.contains("helper_extra="));
    assert!(text.contains("main"));
}

#[test]
/// Verifies fixed-point division by constant zero is diagnosed.
fn rejects_phase22_fixed_division_by_constant_zero() {
    let error = compile_error(
        "pic16f628a",
        "phase22-fixed-div-zero.c",
        "\
__fixed8_8 a;
__fixed8_8 result;
void main(void) {
    result = a / __q8_8(0);
}
",
    );

    assert!(error.contains("division by constant zero"));
}

#[test]
/// Verifies Phase 23 accepts fixed-point ROM calibration tables.
fn compiles_phase23_fixed_rom_objects() {
    let output = compile_source(
        "pic16f628a",
        "phase23-fixed-rom.c",
        "\
const __rom __fixed8_8 table[] = { 1.0q8_8, 1.5q8_8 };
__fixed8_8 result;
void main(void) {
    result = table[1];
}
",
    );

    assert_hex_is_programmable(&output);
}

#[test]
/// Verifies fixed decimal literal suffixes and truncating conversion parse in Phase 23.
fn compiles_phase23_fixed_decimal_literals() {
    let output = compile_source(
        "pic16f628a",
        "phase23-fixed-literals.c",
        "\
__fixed8_8 a = 1.5q8_8;
__ufixed8_8 b = 2.25uq8_8;
__fixed16_16 c = 10.125q16_16;
__ufixed16_16 d = 0.5uq16_16;
void main(void) {
    a = a + 0.25q8_8;
}
",
    );

    assert_hex_is_programmable(&output);
}

#[test]
/// Verifies malformed fixed literals diagnose before semantic lowering.
fn rejects_phase23_malformed_fixed_literal() {
    let error = compile_error(
        "pic16f628a",
        "phase23-malformed-fixed-literal.c",
        "\
__fixed8_8 result = 1.q8_8;
void main(void) {
}
",
    );

    assert!(error.contains("malformed fixed-point literal"));
}

#[test]
/// Verifies unsupported fixed literal suffixes diagnose clearly.
fn rejects_phase23_unsupported_fixed_literal_suffix() {
    let error = compile_error(
        "pic16f628a",
        "phase23-bad-fixed-suffix.c",
        "\
__fixed8_8 result = 1.0q4_4;
void main(void) {
}
",
    );

    assert!(error.contains("unsupported fixed-point literal suffix"));
}

#[test]
/// Verifies fixed literals outside the selected raw range diagnose clearly.
fn rejects_phase23_fixed_literal_out_of_range() {
    let error = compile_error(
        "pic16f628a",
        "phase23-fixed-out-of-range.c",
        "\
__fixed8_8 result = 200.0q8_8;
void main(void) {
}
",
    );

    assert!(error.contains("fixed-point literal is out of range"));
}

#[test]
/// Verifies fixed modulo remains unsupported with an explicit diagnostic.
fn rejects_phase23_fixed_modulo() {
    let error = compile_error(
        "pic16f628a",
        "phase23-fixed-mod.c",
        "\
__fixed8_8 a;
__fixed8_8 b;
__fixed8_8 result;
void main(void) {
    result = a % b;
}
",
    );

    assert!(error.contains("is not supported for fixed-point operands"));
}

#[test]
/// Verifies helper-backed fixed-point operations stay rejected inside interrupt handlers.
fn rejects_phase22_fixed_helper_inside_isr() {
    let error = compile_error(
        "pic16f628a",
        "phase22-fixed-isr-helper.c",
        "\
__fixed8_8 a;
__fixed8_8 b;
__fixed8_8 result;
void __interrupt isr(void) {
    result = a * b;
}
void main(void) {
}
",
    );

    assert!(error.contains("cannot use `Multiply` when it would lower through a runtime helper"));
}

#[test]
/// Verifies checked-in Phase 22 fixed-point examples compile cleanly.
fn phase22_examples_compile_via_picc() {
    let examples = [
        ("pic16f877a", "examples/pic16f628a/fixed_q8_8_basic.c"),
        ("pic16f877a", "examples/pic16f877a/fixed_q8_8_arithmetic.c"),
        ("pic16f877a", "examples/pic16f877a/fixed_sensor_scale.c"),
        ("pic16f877a", "examples/pic16f877a/fixed_struct.c"),
        ("pic16f877a", "examples/pic16f877a/fixed_table.c"),
    ];

    for (target, path) in examples {
        let output = compile_example(target, path);
        assert_hex_is_programmable(&output);
    }
}

#[test]
/// Verifies checked-in Phase 23 fixed-point completeness examples compile cleanly.
fn phase23_examples_compile_via_picc() {
    let examples = [
        ("pic16f628a", "examples/pic16f628a/fixed_literals.c"),
        (
            "pic16f877a",
            "examples/pic16f877a/fixed_q16_16_arithmetic.c",
        ),
        ("pic16f877a", "examples/pic16f877a/fixed_rom_table.c"),
        ("pic16f877a", "examples/pic16f877a/fixed_calibration.c"),
        ("pic16f877a", "examples/pic16f877a/fixed_conversion.c"),
    ];

    for (target, path) in examples {
        let output = compile_example(target, path);
        assert_hex_is_programmable(&output);
    }
}

#[test]
/// Verifies checked-in Phase 24 dynamic Q16.16 examples compile cleanly.
fn phase24_examples_compile_via_picc() {
    let examples = [
        ("pic16f628a", "examples/pic16f628a/fixed_q16_16_dynamic.c"),
        ("pic16f877a", "examples/pic16f877a/fixed_q16_16_muldiv.c"),
        (
            "pic16f877a",
            "examples/pic16f877a/fixed_q16_16_sensor_scale.c",
        ),
    ];

    for (target, path) in examples {
        let output = compile_example(target, path);
        assert_hex_is_programmable(&output);
    }
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
    assert!(makefile.contains("FLASH_CMD ?="));
    assert!(makefile.contains("FLASH_ARGS ?="));
    assert!(makefile.contains("size:"));
    assert!(makefile.contains("sim:"));
    assert!(makefile.contains("flash:"));
    assert!(!makefile.contains("cargo run"));
}

#[test]
/// Verifies the release binary is named `picc`.
fn release_binary_is_picc() {
    let path = picc_bin();
    let stem = path
        .file_stem()
        .and_then(|name| name.to_str())
        .expect("bin stem");
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
    assert!(stdout.contains("Stack summary"));
    assert_hex_is_programmable(&out_hex);
}

#[test]
/// Verifies Phase 18 stack bounds symbols are exposed in the map output.
fn phase18_stack_bounds_symbols_appear_in_map() {
    let output = compile_source(
        "pic16f628a",
        "phase18-stack-bounds.c",
        "\
#include <pic16/pic16f628a.h>
void main(void) {
    TRISB = 0x00;
    PORTB = 0x01;
}
",
    );

    let map = read_artifact(&output, "map");
    assert!(map.contains("__stack_base"));
    assert!(map.contains("__stack_limit"));
    assert!(map.contains("__stack_ptr"));
    assert!(map.contains("__frame_ptr"));
}

#[test]
/// Verifies `--stack-report` and `--stack-report-file` emit Phase 18 stack usage details.
fn phase18_stack_report_prints_and_writes_file() {
    let out_dir = temp_dir_path("cli-stack-report");
    fs::create_dir_all(&out_dir).expect("out dir");
    let out_hex = out_dir.join("stack-report.hex");
    let report_path = out_dir.join("stack-report.txt");
    let output = Command::new(picc_bin())
        .current_dir(repo("."))
        .args([
            "--target",
            "pic16f628a",
            "-Wall",
            "-Wextra",
            "-O2",
            "--stack-report",
            "--stack-report-file",
        ])
        .arg(&report_path)
        .args(["-I", "include", "--emit-asm"])
        .arg("-o")
        .arg(&out_hex)
        .arg("examples/pic16f628a/stack_report.c")
        .output()
        .expect("run picc with stack report");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Stack report"));
    assert!(stdout.contains("Per-function"));
    let report = fs::read_to_string(&report_path).expect("stack report file");
    assert!(report.contains("base: 0x"));
    assert!(report.contains("Recursion policy: rejected in phase 18"));
    assert_hex_is_programmable(&out_hex);
}

#[test]
/// Verifies `--stack-check` emits runtime checks and the overflow trap label.
fn phase18_stack_check_emits_checks_and_trap() {
    let input = temp_file("phase18-stack-check.c");
    fs::write(
        &input,
        "\
#include <pic16/pic16f877a.h>
unsigned int add3(unsigned int a, unsigned int b, unsigned int c) {
    unsigned char local[8];
    local[0] = 1;
    return a + b + c + local[0];
}
void main(void) {
    ADCON1 = 0x06;
    TRISB = 0x00;
    PORTB = add3(1, 2, 3);
}
",
    )
    .expect("fixture");
    let output_hex = temp_file("phase18-stack-check.hex");
    execute(CliOptions {
        command: CliCommand::Compile(CompileCommand {
            target: "pic16f877a".to_string(),
            input,
            output: output_hex.clone(),
            include_dirs: vec![repo("include")],
            defines: BTreeMap::new(),
            optimization: OptimizationLevel::O2,
            artifacts: OutputArtifacts {
                emit_asm: true,
                map: true,
                list_file: true,
                ..OutputArtifacts::default()
            },
            verbose: false,
            opt_report: false,
            stack_check: true,
            stack_report: false,
            stack_report_file: None,
            warning_profile: WarningProfile::default(),
        }),
    })
    .expect("compile with stack check");

    let asm = read_artifact(&output_hex, "asm");
    let map = read_artifact(&output_hex, "map");
    let lst = read_artifact(&output_hex, "lst");
    assert!(asm.contains("phase18 stack check +"));
    assert!(asm.contains("__stack_overflow_trap:"));
    assert!(map.contains("__stack_limit"));
    assert!(lst.contains("phase18 stack check +"));
}

#[test]
/// Verifies stack reports include helper usage, ISR usage, and function-pointer target sets.
fn phase18_stack_report_covers_helpers_isr_and_fnptr() {
    let input = temp_file("phase18-stack-report-details.c");
    let report = temp_file("phase18.stack");
    fs::write(
        &input,
        "\
#include <pic16/pic16f877a.h>
typedef void (*Handler)(void);

void off(void) {
    PORTB = 0x00;
}

void on(void) {
    PORTB = 0x01;
}

Handler table[2] = {off, on};

unsigned int twice(unsigned int value) {
    return value * 2;
}

void __interrupt isr(void) {
    PORTB = 0x55;
}

void main(void) {
    unsigned char index = 1;
    ADCON1 = 0x06;
    TRISB = 0x00;
    table[index]();
    PORTB = twice(7);
}
",
    )
    .expect("fixture");
    execute(CliOptions {
        command: CliCommand::Compile(CompileCommand {
            target: "pic16f877a".to_string(),
            input,
            output: temp_file("phase18-stack-report.hex"),
            include_dirs: vec![repo("include")],
            defines: BTreeMap::new(),
            optimization: OptimizationLevel::O2,
            artifacts: OutputArtifacts::default(),
            verbose: false,
            opt_report: false,
            stack_check: false,
            stack_report: false,
            stack_report_file: Some(report.clone()),
            warning_profile: WarningProfile::default(),
        }),
    })
    .expect("compile with stack report file");

    let text = fs::read_to_string(report).expect("stack report");
    assert!(text.contains("helper_extra="));
    assert!(text.contains("isr frame bytes:"));
    assert!(text.contains("isr context bytes:"));
    assert!(text.contains("indirect fnptr<"));
    assert!(text.contains("off"));
    assert!(text.contains("on"));
}

#[test]
/// Verifies recursion stays rejected even when `--stack-check` is enabled.
fn phase18_recursion_still_rejected_with_stack_check() {
    let input = temp_file("phase18-recursive-checked.c");
    fs::write(
        &input,
        "\
#include <pic16/pic16f877a.h>
unsigned char countdown(unsigned char depth) {
    if (depth == 0) {
        return 0;
    }
    return countdown(depth - 1);
}
void main(void) {
    ADCON1 = 0x06;
    TRISB = 0x00;
    PORTB = countdown(3);
}
",
    )
    .expect("fixture");
    let error = execute(CliOptions {
        command: CliCommand::Compile(CompileCommand {
            target: "pic16f877a".to_string(),
            input,
            output: temp_file("phase18-recursive-checked.hex"),
            include_dirs: vec![repo("include")],
            defines: BTreeMap::new(),
            optimization: OptimizationLevel::O0,
            artifacts: OutputArtifacts::default(),
            verbose: false,
            opt_report: false,
            stack_check: true,
            stack_report: false,
            stack_report_file: None,
            warning_profile: WarningProfile::default(),
        }),
    })
    .expect_err("recursion must stay unsupported");

    let text = format!("{error}");
    assert!(text.contains("recursive call cycle"));
    assert!(text.contains("phase 18"));
}

#[test]
/// Verifies checked-in Phase 18 examples and diagnostics stay wired to the release CLI.
fn phase18_examples_compile_via_picc() {
    let compiling = [
        ("pic16f628a", "examples/pic16f628a/stack_report.c", &[][..]),
        (
            "pic16f877a",
            "examples/pic16f877a/stack_check.c",
            &["--stack-check"][..],
        ),
    ];

    for (target, example, extra_args) in compiling {
        let output = compile_example_via_picc_cli_with_extra_args(target, example, extra_args);
        assert_hex_is_programmable(&output);
        assert!(output.with_extension("map").exists());
        assert!(output.with_extension("lst").exists());
    }

    let recursive = Command::new(picc_bin())
        .current_dir(repo("."))
        .args([
            "--target",
            "pic16f877a",
            "-Wall",
            "-Wextra",
            "-O2",
            "--stack-check",
            "-I",
            "include",
            "-o",
        ])
        .arg(temp_file("phase18-recursive-diag.hex"))
        .arg("examples/pic16f877a/recursive_checked.c")
        .output()
        .expect("run picc recursive diagnostic");
    assert!(!recursive.status.success());
    assert!(String::from_utf8_lossy(&recursive.stderr).contains("recursive call cycle"));

    let mutual = Command::new(picc_bin())
        .current_dir(repo("."))
        .args([
            "--target",
            "pic16f877a",
            "-Wall",
            "-Wextra",
            "-O2",
            "-I",
            "include",
            "-o",
        ])
        .arg(temp_file("phase18-mutual-recursion-diag.hex"))
        .arg("examples/pic16f877a/mutual_recursion_diagnostic.c")
        .output()
        .expect("run picc mutual recursion diagnostic");
    assert!(!mutual.status.success());
    assert!(String::from_utf8_lossy(&mutual.stderr).contains("recursive call cycle"));
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
        ("pic16f877a", "examples/pic16f628a/array_initializer.c"),
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

    assert!(error.contains("duplicate initializer for designator"));
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

    assert!(error.contains("duplicate initializer for designator"));
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

    assert!(
        error.contains("whole-aggregate assignment is not supported inside interrupt handlers")
    );
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

    assert!(
        error.contains("cannot coerce `char*`") || error.contains("cannot coerce `unsigned char*`")
    );
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
    let rom_addr = map_symbol_address(&map, "table [rom, const").expect("rom symbol");

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
    assert!(map.contains("msg [rom, const"));
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
/// Verifies direct ROM byte indexing lowers through the same RETLW-table mechanism as the builtin.
fn compiles_phase14_direct_rom_byte_index() {
    let output = compile_source(
        "pic16f628a",
        "phase14-rom-index.c",
        "\
#include <pic16/pic16f628a.h>
const __rom unsigned char table[] = {1, 2, 3, 4};
void main(void) {
    unsigned char i;
    TRISB = 0x00;
    i = 3;
    PORTB = table[i];
}
",
    );

    let asm = read_artifact(&output, "asm");
    let map = read_artifact(&output, "map");
    let listing = read_artifact(&output, "lst");
    assert_hex_is_programmable(&output);
    assert!(asm.contains("call __romobj"));
    assert!(asm.contains("program-memory ROM tables (Phase 14)"));
    assert!(map.contains("table [rom, const"));
    assert!(listing.contains("__romobj"));
}

#[test]
/// Verifies constant-index ROM reads optimize to inline literals instead of dynamic RETLW dispatch.
fn compiles_phase14_constant_rom_index_optimizes_inline() {
    let output = compile_source(
        "pic16f628a",
        "phase14-rom-const-index.c",
        "\
#include <pic16/pic16f628a.h>
const __rom unsigned char table[] = {1, 2, 3, 4};
void main(void) {
    TRISB = 0x00;
    PORTB = table[2];
}
",
    );

    let asm = read_artifact(&output, "asm");
    assert_hex_is_programmable(&output);
    assert_eq!(count_occurrences(&asm, "call __romobj"), 0);
    assert!(asm.contains("movlw 0x03"));
}

#[test]
/// Verifies direct ROM string indexing works and the trailing null remains visible in the RETLW table.
fn compiles_phase14_rom_string_direct_index() {
    let output = compile_source(
        "pic16f877a",
        "phase14-rom-string-index.c",
        "\
#include <pic16/pic16f877a.h>
const __rom char msg[] = \"OK\";
void main(void) {
    ADCON1 = 0x06;
    TRISB = 0x00;
    PORTB = msg[1];
}
",
    );

    let listing = read_artifact(&output, "lst");
    assert_hex_is_programmable(&output);
    assert!(listing.contains("retlw 0x4F"));
    assert!(listing.contains("retlw 0x4B"));
    assert!(listing.contains("retlw 0x00"));
}

#[test]
/// Verifies unsigned 16-bit ROM tables read correctly and stay little-endian in the RETLW payload.
fn compiles_phase14_rom_table16_unsigned() {
    let output = compile_source(
        "pic16f877a",
        "phase14-rom-table16.c",
        "\
#include <pic16/pic16f877a.h>
const __rom unsigned int table16[] = {100, 200, 300};
unsigned int read_value(unsigned char index) {
    return table16[index];
}
void main(void) {
    unsigned int value;
    ADCON1 = 0x06;
    TRISB = 0x00;
    value = read_value(2);
    PORTB = (unsigned char)value;
}
",
    );

    let asm = read_artifact(&output, "asm");
    let map = read_artifact(&output, "map");
    let listing = read_artifact(&output, "lst");
    assert_hex_is_programmable(&output);
    assert!(asm.contains("call __romobj"));
    assert!(map.contains("table16 [rom, const"));
    assert!(listing.contains("retlw 0x64"));
    assert!(listing.contains("retlw 0xC8"));
    assert!(listing.contains("retlw 0x2C"));
    assert!(listing.contains("retlw 0x01"));
}

#[test]
/// Verifies signed 16-bit ROM tables pack negative values as little-endian two's complement bytes.
fn compiles_phase14_rom_table16_signed() {
    let output = compile_source(
        "pic16f877a",
        "phase14-rom-table16-signed.c",
        "\
#include <pic16/pic16f877a.h>
const __rom int signed_table16[] = {-1, 0, 1};
void main(void) {
    unsigned int value;
    ADCON1 = 0x06;
    TRISB = 0x00;
    value = signed_table16[2];
    PORTB = (unsigned char)value;
}
",
    );

    let listing = read_artifact(&output, "lst");
    assert_hex_is_programmable(&output);
    assert!(listing.contains("retlw 0xFF"));
    assert!(listing.contains("retlw 0x00"));
    assert!(listing.contains("retlw 0x01"));
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

    assert!(
        error.contains("program-memory arrays do not decay to data-space pointers")
            || error.contains("cannot coerce")
    );
}

#[test]
/// Verifies writes through direct ROM indexing are rejected explicitly.
fn reports_phase14_rom_write_rejected() {
    let error = compile_error(
        "pic16f628a",
        "phase14-rom-write.c",
        "\
#include <pic16/pic16f628a.h>
const __rom unsigned char table[] = {1, 2, 3};
void main(void) {
    table[0] = 7;
}
",
    );

    assert!(error.contains("writing to a program-memory array element"));
}

#[test]
/// Verifies taking the address of one ROM element stays rejected until a ROM-pointer model exists.
fn reports_phase14_rom_element_address_rejected() {
    let error = compile_error(
        "pic16f628a",
        "phase14-rom-address.c",
        "\
#include <pic16/pic16f628a.h>
const __rom unsigned char table[] = {1, 2, 3};
unsigned char *ptr;
void main(void) {
    ptr = &table[0];
}
",
    );

    assert!(error.contains("taking the address of a program-memory array element"));
}

#[test]
/// Verifies `__rom_read8` rejects RAM objects as its first argument.
fn reports_phase14_invalid_rom_read8_object_type() {
    let error = compile_error(
        "pic16f628a",
        "phase14-rom-read8-ram.c",
        "\
#include <pic16/pic16f628a.h>
unsigned char table[] = {1, 2, 3};
void main(void) {
    PORTB = __rom_read8(table, 1);
}
",
    );

    assert!(error.contains("first `__rom_read8` argument must be a `const __rom` array object"));
}

#[test]
/// Verifies `__rom_read16` rejects byte-array ROM objects with the wrong element width.
fn reports_phase14_invalid_rom_read16_object_type() {
    let error = compile_error(
        "pic16f877a",
        "phase14-rom-read16-width.c",
        "\
#include <pic16/pic16f877a.h>
const __rom unsigned char table[] = {1, 2, 3, 4};
void main(void) {
    unsigned int value;
    value = __rom_read16(table, 1);
}
",
    );

    assert!(error.contains("`__rom_read16` only supports ROM arrays of `int` or `unsigned int`"));
}

#[test]
/// Verifies dynamic ROM reads remain forbidden inside interrupt handlers in this phase.
fn reports_phase14_dynamic_rom_read_in_isr() {
    let error = compile_error(
        "pic16f877a",
        "phase14-rom-isr.c",
        "\
#include <pic16/pic16f877a.h>
const __rom unsigned char table[] = {1, 2, 3};
unsigned char i;
void __interrupt isr(void) {
    PORTB = table[i];
}
void main(void) {
    ADCON1 = 0x06;
    TRISB = 0x00;
}
",
    );

    assert!(error.contains("cannot perform dynamic ROM reads in phase 14"));
}

#[test]
/// Verifies constant-index ROM reads stay allowed inside ISRs because they lower inline.
fn compiles_phase14_constant_rom_read_in_isr() {
    let output = compile_source(
        "pic16f877a",
        "phase14-rom-isr-const.c",
        "\
#include <pic16/pic16f877a.h>
const __rom unsigned char table[] = {1, 2, 3};
void __interrupt isr(void) {
    PORTB = table[1];
}
void main(void) {
    ADCON1 = 0x06;
    TRISB = 0x00;
}
",
    );

    let asm = read_artifact(&output, "asm");
    assert_hex_is_programmable(&output);
    assert_eq!(count_occurrences(&asm, "call __romobj"), 0);
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

#[test]
/// Verifies checked-in Phase 14 examples compile cleanly through the `picc` CLI.
fn phase14_examples_compile_via_picc() {
    let examples = [
        ("pic16f628a", "examples/pic16f628a/rom_index.c"),
        ("pic16f877a", "examples/pic16f877a/rom_table16.c"),
        ("pic16f877a", "examples/pic16f877a/rom_string_index.c"),
        ("pic16f877a", "examples/pic16f877a/rom_lookup_direct.c"),
    ];

    for (target, example) in examples {
        let output = compile_example_via_picc_cli(target, example);
        assert_hex_is_programmable(&output);
        assert!(output.with_extension("map").exists());
        assert!(output.with_extension("lst").exists());
    }
}

#[test]
/// Verifies union size/offsets and basic bitfield packing appear in the parsed AST artifact.
fn phase15_ast_reports_union_and_bitfield_layout() {
    let output = compile_source(
        "pic16f877a",
        "phase15-layout.c",
        "\
#include <pic16/pic16f877a.h>
union Value {
    unsigned char byte;
    unsigned int word;
};
struct Flags {
    unsigned char ready:1;
    unsigned char error:1;
    unsigned char mode:2;
    unsigned char next;
};
struct Packet {
    unsigned char id;
    union Value value;
};
void main(void) {
    PORTB = 0;
}
",
    );

    let ast = read_artifact(&output, "ast");
    assert!(ast.contains("union Value size=2 fields=byte:unsigned char@0, word:unsigned int@0"));
    assert!(ast.contains("struct Flags size=2 fields=ready:unsigned char@0.0:1"));
    assert!(ast.contains("error:unsigned char@0.1:1"));
    assert!(ast.contains("mode:unsigned char@0.2:2"));
    assert!(ast.contains("next:unsigned char@1"));
    assert!(ast.contains("struct Packet size=3 fields=id:unsigned char@0, value:union#0@1"));
}

#[test]
/// Verifies named union fields, pointer access, and whole-union copy compile through byte-wise lowering.
fn compiles_phase15_union_basic_access_and_copy() {
    let output = compile_source(
        "pic16f628a",
        "phase15-union-basic.c",
        "\
#include <pic16/pic16f628a.h>
union Value {
    unsigned char byte;
    unsigned int word;
};
void main(void) {
    union Value a;
    union Value b;
    union Value *ptr;
    TRISB = 0x00;
    a.byte = 3;
    b.word = 1000;
    a = b;
    ptr = &a;
    PORTB = ptr->byte;
}
",
    );

    let asm = read_artifact(&output, "asm");
    assert_hex_is_programmable(&output);
    assert!(asm.contains("movwf"));
}

#[test]
/// Verifies nested named unions inside structs remain addressable with composed offsets.
fn compiles_phase15_nested_union_in_struct_access() {
    let output = compile_source(
        "pic16f877a",
        "phase15-union-struct.c",
        "\
#include <pic16/pic16f877a.h>
union Payload {
    unsigned char byte;
    unsigned int word;
};
struct Packet {
    unsigned char id;
    union Payload payload;
};
void main(void) {
    struct Packet packet;
    struct Packet *ptr;
    ADCON1 = 0x06;
    TRISB = 0x00;
    ptr = &packet;
    ptr->payload.word = 0x0034;
    PORTB = ptr->payload.byte + ptr->id;
}
",
    );

    assert_hex_is_programmable(&output);
}

#[test]
/// Verifies first-field and designated union initializers lower through zero-fill plus selected-field overlay.
fn compiles_phase15_union_initializer_variants() {
    let output = compile_source(
        "pic16f877a",
        "phase15-union-init.c",
        "\
#include <pic16/pic16f877a.h>
union Value {
    unsigned char byte;
    unsigned int word;
};
union Value first = {3};
union Value selected = {.word = 1000};
void main(void) {
    ADCON1 = 0x06;
    TRISB = 0x00;
    PORTB = first.byte + (unsigned char)selected.word;
}
",
    );

    let listing = read_artifact(&output, "lst");
    assert_hex_is_programmable(&output);
    assert!(listing.contains("0x03"));
    assert!(listing.contains("0xE8"));
}

#[test]
/// Verifies basic unsigned bitfield reads and writes lower to real mask/shift operations.
fn compiles_phase15_bitfield_flags() {
    let output = compile_source(
        "pic16f877a",
        "phase15-bitfield-flags.c",
        "\
#include <pic16/pic16f877a.h>
struct Flags {
    unsigned char ready:1;
    unsigned char error:1;
    unsigned char mode:2;
    unsigned char spare:4;
};
void main(void) {
    struct Flags flags;
    ADCON1 = 0x06;
    TRISB = 0x00;
    flags.ready = 1;
    flags.error = 0;
    flags.mode = 2;
    if (flags.ready) {
        PORTB = flags.mode;
    }
}
",
    );

    let asm = read_artifact(&output, "asm");
    assert_hex_is_programmable(&output);
    assert!(
        asm.contains("andwf")
            || asm.contains("iorwf")
            || asm.contains("bsf")
            || asm.contains("bcf")
    );
}

#[test]
/// Verifies incompatible union assignment is rejected explicitly.
fn reports_phase15_incompatible_union_assignment() {
    let error = compile_error(
        "pic16f628a",
        "phase15-union-assign-error.c",
        "\
#include <pic16/pic16f628a.h>
union A { unsigned char byte; };
union B { unsigned char byte; };
void main(void) {
    union A a;
    union B b;
    a = b;
}
",
    );

    assert!(error.contains("cannot assign incompatible union type"));
}

#[test]
/// Verifies duplicate union field names are rejected during aggregate parsing.
fn reports_phase15_duplicate_union_field() {
    let error = compile_error(
        "pic16f628a",
        "phase15-dup-union-field.c",
        "\
#include <pic16/pic16f628a.h>
union Value {
    unsigned char byte;
    unsigned int byte;
};
void main(void) {
}
",
    );

    assert!(error.contains("duplicate union field `byte`"));
}

#[test]
/// Verifies assignment to a const union object stays rejected through the ordinary const-object rule.
fn reports_phase15_const_union_assignment() {
    let error = compile_error(
        "pic16f877a",
        "phase15-const-union-assign.c",
        "\
#include <pic16/pic16f877a.h>
union Value {
    unsigned char byte;
    unsigned int word;
};
void main(void) {
    const union Value value = {.word = 1};
    value.word = 2;
}
",
    );

    assert!(error.contains("assignment to const object"));
}

#[test]
/// Verifies taking the address of a bitfield stays rejected because bitfields are not addressable objects.
fn reports_phase15_bitfield_address_rejected() {
    let error = compile_error(
        "pic16f877a",
        "phase15-bitfield-address.c",
        "\
#include <pic16/pic16f877a.h>
struct Flags {
    unsigned char ready:1;
    unsigned char mode:2;
};
unsigned char *ptr;
void main(void) {
    struct Flags flags;
    ptr = &flags.ready;
}
",
    );

    assert!(error.contains("taking the address of a bitfield"));
}

#[test]
/// Verifies invalid bitfield base types are diagnosed clearly.
fn reports_phase15_invalid_bitfield_base_type() {
    let error = compile_error(
        "pic16f877a",
        "phase15-bitfield-base.c",
        "\
#include <pic16/pic16f877a.h>
struct Flags {
    unsigned char *ptr:1;
};
void main(void) {
}
",
    );

    assert!(error.contains("bitfield `ptr` must use `unsigned char` or `unsigned int`"));
}

#[test]
/// Verifies anonymous union fields remain deferred and are rejected clearly.
fn reports_phase15_anonymous_union_field() {
    let error = compile_error(
        "pic16f877a",
        "phase15-anon-union-field.c",
        "\
#include <pic16/pic16f877a.h>
struct Packet {
    union {
        unsigned char byte;
        unsigned int word;
    };
};
void main(void) {
}
",
    );

    assert!(
        error.contains("anonymous nested struct/union/enum fields are not supported in phase 15")
    );
}

#[test]
/// Verifies checked-in Phase 15 examples compile cleanly through the `picc` CLI.
fn phase15_examples_compile_via_picc() {
    let examples = [
        ("pic16f628a", "examples/pic16f628a/union_basic.c"),
        ("pic16f877a", "examples/pic16f877a/union_struct_nested.c"),
        ("pic16f877a", "examples/pic16f877a/bitfield_flags.c"),
        ("pic16f877a", "examples/pic16f877a/bitfield_register_like.c"),
        ("pic16f877a", "examples/pic16f877a/union_initializer.c"),
    ];

    for (target, example) in examples {
        let output = compile_example_via_picc_cli(target, example);
        assert_hex_is_programmable(&output);
        assert!(output.with_extension("map").exists());
        assert!(output.with_extension("lst").exists());
    }
}

#[test]
/// Verifies multidimensional aggregate layout renders row-major sizes and composed field offsets.
fn phase16_ast_reports_multidimensional_layout() {
    let output = compile_source(
        "pic16f877a",
        "phase16-layout.c",
        "\
#include <pic16/pic16f877a.h>
struct Font {
    unsigned char glyph[2][3];
    unsigned char width;
};
unsigned char matrix[2][3];
void main(void) {
    struct Font font;
    PORTB = font.glyph[1][2] + matrix[1][2];
}
",
    );

    let ast = read_artifact(&output, "ast");
    assert!(ast.contains("glyph:unsigned char[2][3]@0"));
    assert!(ast.contains("width:unsigned char@"));
    assert!(ast.contains("global unsigned char[2][3] matrix"));
}

#[test]
/// Verifies nested initializer lists flatten and zero-fill multidimensional arrays.
fn compiles_phase16_multidimensional_initializer() {
    let output = compile_source(
        "pic16f877a",
        "phase16-matrix-init.c",
        "\
#include <pic16/pic16f877a.h>
unsigned char matrix[2][3] = {
    {1},
    {4, 5}
};
void main(void) {
    ADCON1 = 0x06;
    TRISB = 0x00;
    PORTB = matrix[1][1];
}
",
    );

    let map = read_artifact(&output, "map");
    assert_hex_is_programmable(&output);
    assert!(map.contains("matrix"));
}

#[test]
/// Verifies multidimensional arrays inside structs support composed field plus index offsets.
fn compiles_phase16_struct_matrix_field_access() {
    let output = compile_source(
        "pic16f877a",
        "phase16-struct-matrix.c",
        "\
#include <pic16/pic16f877a.h>
struct Font {
    unsigned char glyph[2][3];
};
void main(void) {
    struct Font font;
    struct Font *ptr = &font;
    ptr->glyph[1][2] = 7;
    ADCON1 = 0x06;
    TRISB = 0x00;
    PORTB = font.glyph[1][2];
}
",
    );

    assert_hex_is_programmable(&output);
    assert!(read_artifact(&output, "asm").contains("main"));
}

#[test]
/// Verifies chained designators resolve nested field/index targets.
fn compiles_phase16_chained_designators() {
    let output = compile_source(
        "pic16f877a",
        "phase16-chained-designators.c",
        "\
#include <pic16/pic16f877a.h>
struct Font {
    unsigned char glyph[2][3];
};
struct Font font = {
    .glyph[0][1] = 5,
    .glyph[1][2] = 9
};
void main(void) {
    ADCON1 = 0x06;
    TRISB = 0x00;
    PORTB = font.glyph[1][2];
}
",
    );

    assert_hex_is_programmable(&output);
}

#[test]
/// Verifies duplicate chained designators diagnose the repeated final target clearly.
fn reports_phase16_duplicate_chained_designator() {
    let error = compile_error(
        "pic16f877a",
        "phase16-dup-chained-designator.c",
        "\
struct Font {
    unsigned char glyph[2][3];
};
struct Font font = {
    .glyph[1][2] = 7,
    .glyph[1][2] = 8
};
void main(void) {
}
",
    );

    assert!(error.contains("duplicate initializer for designator .glyph"));
}

#[test]
/// Verifies multidimensional ROM arrays remain deferred and diagnose clearly.
fn reports_phase16_rom_multidimensional_array_unsupported() {
    let error = compile_error(
        "pic16f877a",
        "phase16-rom-matrix.c",
        "\
const __rom unsigned char table[2][3] = {
    {1, 2, 3},
    {4, 5, 6}
};
void main(void) {
}
",
    );

    assert!(error.contains("unsupported multidimensional ROM array"));
}

#[test]
/// Verifies constant multidimensional indexing remains ISR-safe when it stays inline.
fn compiles_phase16_constant_multidimensional_index_in_isr() {
    let output = compile_source(
        "pic16f877a",
        "phase16-isr-const-matrix.c",
        "\
#include <pic16/pic16f877a.h>
unsigned char matrix[2][3] = {
    {1, 2, 3},
    {4, 5, 6}
};
void __interrupt isr(void) {
    PORTB = matrix[1][2];
}
void main(void) {
    ADCON1 = 0x06;
    TRISB = 0x00;
}
",
    );

    assert_hex_is_programmable(&output);
}

#[test]
/// Verifies helper-requiring dynamic multidimensional indexing is rejected inside ISR code.
fn reports_phase16_dynamic_multidimensional_index_in_isr() {
    let error = compile_error(
        "pic16f877a",
        "phase16-isr-dyn-matrix.c",
        "\
#include <pic16/pic16f877a.h>
unsigned char matrix[2][3];
void __interrupt isr(void) {
    unsigned char i = 1;
    PORTB = matrix[i][2];
}
void main(void) {
    ADCON1 = 0x06;
    TRISB = 0x00;
}
",
    );

    assert!(error.contains("cannot use `Multiply` when it would lower through a runtime helper"));
}

#[test]
/// Verifies checked-in Phase 16 examples compile cleanly through the `picc` CLI.
fn phase16_examples_compile_via_picc() {
    let examples = [
        ("pic16f628a", "examples/pic16f628a/matrix_basic.c"),
        ("pic16f877a", "examples/pic16f877a/matrix_initializer.c"),
        ("pic16f877a", "examples/pic16f877a/struct_matrix_field.c"),
        ("pic16f877a", "examples/pic16f877a/chained_designators.c"),
        ("pic16f877a", "examples/pic16f877a/matrix_switch_state.c"),
    ];

    for (target, example) in examples {
        let output = compile_example_via_picc_cli(target, example);
        assert_hex_is_programmable(&output);
        assert!(output.with_extension("map").exists());
        assert!(output.with_extension("lst").exists());
    }
}

#[test]
/// Verifies Phase 25 resource summaries are embedded in map and listing artifacts.
fn phase25_map_and_listing_include_resource_summary() {
    let output = compile_source(
        "pic16f628a",
        "phase25-resource-summary.c",
        "\
unsigned char result;
void main(void) {
    result = 5;
}
",
    );

    let map = read_artifact(&output, "map");
    let listing = read_artifact(&output, "lst");
    assert!(map.contains("Memory Summary"));
    assert!(map.contains("Program words:"));
    assert!(map.contains("Data RAM:"));
    assert!(listing.contains("; Resource summary"));
    assert!(listing.contains("; Program words:"));
}

#[test]
/// Verifies Phase 31 page metadata and page-safe control-flow annotations reach artifacts.
fn phase31_map_and_listing_include_page_safety_metadata() {
    let output = compile_source(
        "pic16f877a",
        "phase31-page-safety-artifacts.c",
        r#"
float a;
float b;
unsigned char result;

void main(void) {
    a = 3.0f;
    b = 2.0f;
    if (a > b) {
        result = 1;
    } else {
        result = 0;
    }
}
"#,
    );

    let map = read_artifact(&output, "map");
    let listing = read_artifact(&output, "lst");
    assert!(map.contains("page="));
    assert!(map.contains("__rt_f32_cmp"));
    assert!(listing.contains("page-safe control-flow target"));
    assert!(listing.contains("__rt_f32_cmp"));
}

#[test]
/// Verifies Phase 32 reports page layout and linker relaxation data.
fn phase32_reports_page_layout_and_relaxation() {
    let input = temp_file("phase32-layout-report.c");
    fs::write(
        &input,
        r#"
float a;
float b;
unsigned char result;

void main(void) {
    a = 3.0f;
    b = 2.0f;
    if (a > b) {
        result = 1;
    } else {
        result = 0;
    }
}
"#,
    )
    .expect("fixture");
    let output = temp_file("phase32-layout-report.hex");
    let memory_report = temp_file("phase32-layout-report.mem");
    execute(CliOptions {
        command: CliCommand::Compile(CompileCommand {
            target: "pic16f877a".to_string(),
            input,
            output: output.clone(),
            include_dirs: vec![repo("include")],
            defines: BTreeMap::new(),
            optimization: OptimizationLevel::O2,
            artifacts: OutputArtifacts {
                map: true,
                list_file: true,
                size: true,
                memory_report: true,
                memory_report_file: Some(memory_report.clone()),
                ..OutputArtifacts::default()
            },
            verbose: false,
            opt_report: false,
            stack_check: false,
            stack_report: false,
            stack_report_file: None,
            warning_profile: WarningProfile::default(),
        }),
    })
    .expect("compile phase32 layout report");

    let map = read_artifact(&output, "map");
    let listing = read_artifact(&output, "lst");
    let memory = fs::read_to_string(memory_report).expect("memory report");
    assert!(map.contains("Code Layout"));
    assert!(map.contains("page 0: used="));
    assert!(map.contains("Page setup relaxation"));
    assert!(listing.contains("page-safe control-flow target"));
    assert!(memory.contains("Page layout"));
    assert!(memory.contains("page setup relaxation"));
    assert!(memory.contains("__rt_f32_cmp"));
}

#[test]
/// Verifies Phase 33 helper cost reports include categories, graph data, and runtime profiles.
fn phase33_runtime_helper_report_and_profile_cli() {
    let out_dir = temp_dir_path("phase33-runtime-report");
    fs::create_dir_all(&out_dir).expect("out dir");
    let out_hex = out_dir.join("runtime.hex");
    let report_path = out_dir.join("runtime.mem");
    let source = out_dir.join("runtime.c");
    fs::write(
        &source,
        r#"
float raw;
float gain;
unsigned char result;

void main(void) {
    raw = 1.5f;
    gain = 2.0f;
    if (raw < gain) {
        result = 1;
    } else {
        result = 0;
    }
}
"#,
    )
    .expect("fixture");

    let output = Command::new(picc_bin())
        .current_dir(repo("."))
        .args([
            "--target",
            "pic16f877a",
            "-Wall",
            "-Wextra",
            "-O2",
            "-I",
            "include",
            "--runtime-profile",
            "small",
            "--size",
            "--memory-report",
            "--memory-report-file",
        ])
        .arg(&report_path)
        .args(["--map", "--list-file", "-o"])
        .arg(&out_hex)
        .arg(&source)
        .output()
        .expect("run picc runtime report");

    if !output.status.success() {
        panic!(
            "picc failed: stdout={:?} stderr={:?}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Runtime helpers:"));
    assert!(stdout.contains("float:"));
    let report = fs::read_to_string(report_path).expect("memory report");
    assert!(report.contains("Runtime Helper Contributors"));
    assert!(report.contains("Runtime Helper Dependency Graph"));
    assert!(report.contains("__rt_f32_cmp"));
    assert!(report.contains("category=float"));
    let map = read_artifact(&out_hex, "map");
    assert!(map.contains("Runtime helper words by category"));
    assert!(map.contains("    float"));
    assert_hex_is_programmable(&out_hex);
}

#[test]
/// Verifies Phase 33 does not emit helpers just because runtime-capable types are present.
fn phase33_prunes_unused_runtime_helpers() {
    let input = temp_file("phase33-pruned-helpers.c");
    fs::write(
        &input,
        r#"
float f = 1.5f;
long l = 3L;
__fixed16_16 q = 1.0q16_16;

void main(void) {
}
"#,
    )
    .expect("fixture");
    let output = temp_file("phase33-pruned-helpers.hex");
    execute(CliOptions {
        command: CliCommand::Compile(CompileCommand {
            target: "pic16f877a".to_string(),
            input,
            output: output.clone(),
            include_dirs: vec![repo("include")],
            defines: BTreeMap::new(),
            optimization: OptimizationLevel::O2,
            artifacts: OutputArtifacts {
                map: true,
                list_file: true,
                size: true,
                memory_report: true,
                ..OutputArtifacts::default()
            },
            verbose: false,
            opt_report: false,
            stack_check: false,
            stack_report: false,
            stack_report_file: None,
            warning_profile: WarningProfile::default(),
        }),
    })
    .expect("compile pruned helper fixture");

    let map = read_artifact(&output, "map");
    assert!(!map.contains("__rt_f32_add"));
    assert!(!map.contains("__rt_f32_mul"));
    assert!(!map.contains("__rt_div_q16_16"));
    assert!(!map.contains("__rt_div_u32"));
    assert_hex_is_programmable(&output);
}

#[test]
/// Verifies Phase 33 runtime-size examples remain buildable.
fn phase33_runtime_size_examples_compile_via_picc() {
    for (target, example) in [
        ("pic16f877a", "examples/pic16f877a/runtime_size_float.c"),
        ("pic16f877a", "examples/pic16f877a/runtime_size_fixed.c"),
        ("pic16f628a", "examples/pic16f628a/runtime_size_small.c"),
    ] {
        let output = compile_example_via_picc_cli_with_extra_args(
            target,
            example,
            &["--size", "--memory-report", "--runtime-profile", "balanced"],
        );
        assert_hex_is_programmable(&output);
    }
}

#[test]
/// Verifies Phase 34 `small` selects a shared u32 div/mod core and reduces code size.
fn phase34_small_profile_compacts_u32_divmod_helpers() {
    let input = "examples/pic16f877a/runtime_profile_small_divmod.c";
    let (balanced_hex, balanced_stdout, balanced_report) =
        compile_profile_size_report("balanced", input, "phase34-balanced-divmod", &[]);
    let (small_hex, small_stdout, small_report) =
        compile_profile_size_report("small", input, "phase34-small-divmod", &["--stack-report"]);

    let balanced_words = parse_program_words(&balanced_stdout);
    let small_words = parse_program_words(&small_stdout);
    assert!(
        small_words < balanced_words,
        "small profile should reduce div/mod program words: small={small_words} balanced={balanced_words}"
    );
    assert!(small_stdout.contains("Runtime profile: small"));
    assert!(balanced_stdout.contains("Runtime profile: balanced"));
    assert!(small_stdout.contains("helper_extra="));

    assert!(small_report.contains("__rt_u32_divmod_core"));
    assert!(small_report.contains("variant=small"));
    assert!(small_report.contains("deps=__rt_u32_divmod_core"));
    assert!(!balanced_report.contains("__rt_u32_divmod_core"));

    let small_map = read_artifact(&small_hex, "map");
    let small_lst = read_artifact(&small_hex, "lst");
    assert!(small_map.contains("__rt_u32_divmod_core"));
    assert!(small_lst.contains("helper=__rt_u32_divmod_core"));
    assert_hex_is_programmable(&balanced_hex);
    assert_hex_is_programmable(&small_hex);
}

#[test]
/// Verifies Phase 34 runtime-profile examples compile and keep HEX validation intact.
fn phase34_runtime_profile_examples_compile_via_picc() {
    for example in [
        "examples/pic16f877a/runtime_profile_small_divmod.c",
        "examples/pic16f877a/runtime_profile_small_fixed.c",
        "examples/pic16f877a/runtime_profile_compare.c",
    ] {
        let output = compile_example_via_picc_cli_with_extra_args(
            "pic16f877a",
            example,
            &["--size", "--memory-report", "--runtime-profile", "small"],
        );
        assert_hex_is_programmable(&output);
    }
}

#[test]
/// Verifies Phase 35 `small` compacts signed Q16.16 division through the unsigned helper.
fn phase35_small_profile_compacts_q16_div_helpers() {
    let input = "examples/pic16f877a/runtime_profile_small_q16.c";
    let (balanced_hex, balanced_stdout, balanced_report) =
        compile_profile_size_report("balanced", input, "phase35-balanced-q16-div", &[]);
    let (small_hex, small_stdout, small_report) =
        compile_profile_size_report("small", input, "phase35-small-q16-div", &["--stack-report"]);

    let balanced_words = parse_program_words(&balanced_stdout);
    let small_words = parse_program_words(&small_stdout);
    assert!(
        small_words < balanced_words,
        "small profile should reduce q16 division words: small={small_words} balanced={balanced_words}"
    );
    assert!(small_stdout.contains("helper_extra="));
    assert!(small_report.contains("__rt_div_q16_16"));
    assert!(small_report.contains("variant=small"));
    assert!(small_report.contains("deps=__rt_div_uq16_16"));
    assert!(!balanced_report.contains("deps=__rt_div_uq16_16"));
    assert_hex_is_programmable(&balanced_hex);
    assert_hex_is_programmable(&small_hex);
}

#[test]
/// Verifies Phase 35 `small` compacts signed Q16.16 multiplication through the unsigned helper.
fn phase35_small_profile_compacts_q16_mul_helpers() {
    let source = r#"
__ufixed16_16 ua;
__ufixed16_16 ub;
__ufixed16_16 ur;
__fixed16_16 sa;
__fixed16_16 sb;
__fixed16_16 sr;

void main(void) {
    ua = 1.5uq16_16;
    ub = 2.0uq16_16;
    ur = ua * ub;
    sa = -1.5q16_16;
    sb = 2.0q16_16;
    sr = sa * sb;
}
"#;
    let (small_hex, small_stdout, small_report) =
        compile_profile_source_size_report("small", "phase35-small-q16-mul", source, &[]);

    let small_words = parse_program_words(&small_stdout);
    assert!(
        small_words < 8192,
        "small profile should keep q16 multiply fixture within target: small={small_words}"
    );
    assert!(small_report.contains("__rt_mul_q16_16"));
    assert!(small_report.contains("variant=small"));
    assert!(small_report.contains("deps=__rt_mul_uq16_16"));
    assert_hex_is_programmable(&small_hex);
}

#[test]
/// Verifies Phase 35 `small` compacts float subtraction through float addition.
fn phase35_small_profile_compacts_float_sub_helper() {
    let input = "examples/pic16f877a/runtime_profile_small_float.c";
    let (balanced_hex, balanced_stdout, balanced_report) =
        compile_profile_size_report("balanced", input, "phase35-balanced-float", &[]);
    let (small_hex, small_stdout, small_report) =
        compile_profile_size_report("small", input, "phase35-small-float", &["--stack-report"]);

    let balanced_words = parse_program_words(&balanced_stdout);
    let small_words = parse_program_words(&small_stdout);
    assert!(
        small_words < balanced_words,
        "small profile should reduce float add/sub words: small={small_words} balanced={balanced_words}"
    );
    assert!(small_stdout.contains("helper_extra="));
    assert!(small_report.contains("__rt_f32_sub"));
    assert!(small_report.contains("variant=small"));
    assert!(small_report.contains("deps=__rt_f32_add"));
    assert!(!balanced_report.contains("deps=__rt_f32_add"));
    assert_hex_is_programmable(&balanced_hex);
    assert_hex_is_programmable(&small_hex);
}

#[test]
/// Verifies Phase 35 profiles remain monotonic for conversion and mixed numeric programs.
fn phase35_small_profile_size_comparisons_cover_conversion_and_mixed() {
    let conversion = r#"
long lvalue;
long lresult;
float fvalue;

void main(void) {
    lvalue = -1000L;
    fvalue = (float)lvalue;
    lresult = (long)fvalue;
}
"#;
    let (balanced_hex, balanced_stdout, _) = compile_profile_source_size_report(
        "balanced",
        "phase35-balanced-conversion",
        conversion,
        &[],
    );
    let (small_hex, small_stdout, _) =
        compile_profile_source_size_report("small", "phase35-small-conversion", conversion, &[]);
    assert!(parse_program_words(&small_stdout) <= parse_program_words(&balanced_stdout));
    assert_hex_is_programmable(&balanced_hex);
    assert_hex_is_programmable(&small_hex);

    let (balanced_hex, balanced_stdout, _) = compile_profile_size_report(
        "balanced",
        "examples/pic16f877a/runtime_profile_small_mixed_numeric.c",
        "phase35-balanced-mixed",
        &[],
    );
    let (small_hex, small_stdout, _) = compile_profile_size_report(
        "small",
        "examples/pic16f877a/runtime_profile_small_mixed_numeric.c",
        "phase35-small-mixed",
        &[],
    );
    assert!(parse_program_words(&small_stdout) < parse_program_words(&balanced_stdout));
    assert_hex_is_programmable(&balanced_hex);
    assert_hex_is_programmable(&small_hex);
}

#[test]
/// Verifies Phase 35 runtime-profile examples compile and keep HEX validation intact.
fn phase35_runtime_profile_examples_compile_via_picc() {
    for example in [
        "examples/pic16f877a/runtime_profile_small_q16.c",
        "examples/pic16f877a/runtime_profile_small_float.c",
        "examples/pic16f877a/runtime_profile_small_mixed_numeric.c",
    ] {
        let output = compile_example_via_picc_cli_with_extra_args(
            "pic16f877a",
            example,
            &["--size", "--memory-report", "--runtime-profile", "small"],
        );
        assert_hex_is_programmable(&output);
    }
}

#[test]
/// Verifies unsupported runtime profile names are rejected at CLI parsing.
fn phase33_rejects_unknown_runtime_profile() {
    let output = Command::new(picc_bin())
        .current_dir(repo("."))
        .args([
            "--target",
            "pic16f877a",
            "--runtime-profile",
            "tiny",
            "-o",
            "ignored.hex",
            "examples/pic16f877a/runtime_size_fixed.c",
        ])
        .output()
        .expect("run picc");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("unsupported runtime profile"));
}

#[test]
/// Verifies `--size`, `--memory-report`, and `--memory-report-file` expose helper cost.
fn phase25_resource_report_cli_outputs_helper_contribution() {
    let out_dir = temp_dir_path("phase25-memory-report");
    fs::create_dir_all(&out_dir).expect("out dir");
    let out_hex = out_dir.join("fixed.hex");
    let report_path = out_dir.join("fixed.mem");
    let output = Command::new(picc_bin())
        .current_dir(repo("."))
        .args([
            "--target",
            "pic16f877a",
            "-Wall",
            "-Wextra",
            "-O2",
            "-I",
            "include",
            "--size",
            "--memory-report",
            "--memory-report-file",
        ])
        .arg(&report_path)
        .args(["--map", "--list-file", "-o"])
        .arg(&out_hex)
        .arg("examples/pic16f877a/memory_report_fixed.c")
        .output()
        .expect("run picc memory report");

    if !output.status.success() {
        panic!(
            "picc failed: stdout={:?} stderr={:?}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Target: PIC16F877A"));
    assert!(stdout.contains("Memory report"));
    assert!(stdout.contains("__rt_mul_q16_16"));
    let report = fs::read_to_string(report_path).expect("memory report file");
    assert!(report.contains("Runtime helper contribution"));
    assert!(report.contains("__rt_mul_q16_16"));
    assert!(report.contains("Largest contributors"));
    assert_hex_is_programmable(&out_hex);
}

#[test]
/// Verifies Phase 25 data RAM overflow diagnostics are explicit.
fn phase25_reports_data_ram_overflow() {
    let error = compile_error(
        "pic16f628a",
        "phase25-ram-overflow.c",
        "\
unsigned char big[100];
void main(void) {
    big[0] = 1;
}
",
    );

    assert!(error.contains("data RAM overflow"));
    assert!(error.contains("not enough allocatable RAM"));
}

#[test]
/// Verifies Phase 25 stack-region overflow diagnostics are explicit.
fn phase25_reports_stack_region_overflow() {
    let error = compile_error(
        "pic16f628a",
        "phase25-stack-overflow.c",
        "\
void main(void) {
    unsigned char local[80];
    local[0] = 1;
}
",
    );

    assert!(error.contains("stack region overflow"));
    assert!(error.contains("Phase 4 software stack needs"));
}

#[test]
/// Verifies Phase 25 still rejects ROM tables that cannot fit the RETLW page model.
fn phase25_reports_rom_table_too_large() {
    let values = vec!["1"; 256].join(", ");
    let source = format!(
        "\
const __rom unsigned char table[] = {{ {values} }};
void main(void) {{
}}
"
    );
    let error = compile_error("pic16f628a", "phase25-rom-too-large.c", &source);

    assert!(error.contains("too large for one phase 14 RETLW"));
}

#[test]
/// Verifies program-memory overflow is a hard error when resource fitting is requested.
fn phase25_reports_program_memory_overflow_when_size_requested() {
    let out_hex = temp_file("phase25-overflow.hex");
    let output = Command::new(picc_bin())
        .current_dir(repo("."))
        .args([
            "--target",
            "pic16f628a",
            "-Wall",
            "-Wextra",
            "-O2",
            "-I",
            "include",
            "--size",
            "-o",
        ])
        .arg(&out_hex)
        .arg("examples/pic16f628a/stack_abi.c")
        .output()
        .expect("run picc overflow");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("program memory overflow for target pic16f628a"));
}

#[test]
/// Verifies checked-in Phase 25 examples compile through the `picc` CLI.
fn phase25_examples_compile_via_picc() {
    let examples = [
        ("pic16f628a", "examples/pic16f628a/size_report_small.c"),
        ("pic16f877a", "examples/pic16f877a/memory_report_fixed.c"),
        ("pic16f877a", "examples/pic16f877a/memory_report_rom.c"),
    ];

    for (target, example) in examples {
        let output = compile_example_via_picc_cli_with_extra_args(
            target,
            example,
            &["--size", "--memory-report"],
        );
        assert_hex_is_programmable(&output);
        assert!(output.with_extension("map").exists());
        assert!(output.with_extension("lst").exists());
    }
}

#[test]
/// Verifies Phase 26 raw `__config(...)` emits the requested config word in HEX/map/listing.
fn phase26_raw_config_word_emits_to_artifacts() {
    let output = compile_source(
        "pic16f628a",
        "phase26-raw-config.c",
        "\
__config(0x3F18);
unsigned char result;
void main(void) {
    result = 1;
}
",
    );

    let hex = read_hex_bytes(&output);
    assert_eq!(hex.get(&(0x2007 * 2)).copied(), Some(0x18));
    assert_eq!(hex.get(&(0x2007 * 2 + 1)).copied(), Some(0x3F));
    let map = read_artifact(&output, "map");
    let listing = read_artifact(&output, "lst");
    assert!(map.contains("Config word: 0x3F18 @ 0x2007"));
    assert!(listing.contains("Config word: 0x3F18 @ 0x2007"));
}

#[test]
/// Verifies Phase 26 symbolic config pragmas are accepted.
fn phase26_symbolic_config_pragmas_compile() {
    let output = compile_source(
        "pic16f628a",
        "phase26-symbolic-config.c",
        "\
#pragma config FOSC = INTRC_NOCLKOUT
#pragma config WDTE = OFF
#pragma config PWRTE = ON
#pragma config MCLRE = ON
#pragma config BOREN = ON
#pragma config LVP = OFF
#pragma config CPD = OFF
#pragma config CP = OFF
unsigned char result;
void main(void) {
    result = 2;
}
",
    );

    assert_hex_is_programmable(&output);
    let map = read_artifact(&output, "map");
    assert!(map.contains("Config word:"));
}

#[test]
/// Verifies Phase 26 rejects duplicate config settings.
fn phase26_rejects_duplicate_config_setting() {
    let error = compile_error(
        "pic16f628a",
        "phase26-config-duplicate.c",
        "\
#pragma config WDTE = OFF
#pragma config WDTE = ON
void main(void) {}
",
    );

    assert!(error.contains("duplicate config setting `WDTE`"));
}

#[test]
/// Verifies Phase 26 rejects unknown config fields and values.
fn phase26_rejects_unknown_config_field_and_value() {
    let field_error = compile_error(
        "pic16f628a",
        "phase26-config-field.c",
        "\
#pragma config NOPE = OFF
void main(void) {}
",
    );
    assert!(field_error.contains("unknown config field `NOPE`"));

    let value_error = compile_error(
        "pic16f628a",
        "phase26-config-value.c",
        "\
#pragma config WDTE = MAYBE
void main(void) {}
",
    );
    assert!(value_error.contains("unknown config value `MAYBE`"));
}

#[test]
/// Verifies Phase 26 HEX validation report can be printed.
fn phase26_verify_hex_prints_report() {
    let out_dir = temp_dir_path("phase26-verify-hex");
    fs::create_dir_all(&out_dir).expect("out dir");
    let out_hex = out_dir.join("verify.hex");
    let output = Command::new(picc_bin())
        .current_dir(repo("."))
        .args([
            "--target",
            "pic16f628a",
            "-Wall",
            "-Wextra",
            "-O2",
            "-I",
            "include",
            "--verify-hex",
            "-o",
        ])
        .arg(&out_hex)
        .arg("examples/pic16f628a/blink.c")
        .output()
        .expect("run picc verify hex");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("HEX validation"));
    assert!(stdout.contains("checksum: ok"));
    assert_hex_is_programmable(&out_hex);
}

#[test]
/// Verifies invalid target memory output is rejected even without `--size`.
fn phase26_rejects_program_overflow_without_size() {
    let out_hex = temp_file("phase26-overflow.hex");
    let output = Command::new(picc_bin())
        .current_dir(repo("."))
        .args([
            "--target",
            "pic16f628a",
            "-Wall",
            "-Wextra",
            "-O2",
            "-I",
            "include",
            "-o",
        ])
        .arg(&out_hex)
        .arg("examples/pic16f628a/stack_abi.c")
        .output()
        .expect("run picc overflow");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("outside target pic16f628a range"));
}

#[test]
/// Verifies Phase 26 programmer command printing is configurable and does not need input source.
fn phase26_print_program_command_works() {
    let output = Command::new(picc_bin())
        .current_dir(repo("."))
        .args([
            "--print-program-command",
            "--program-cmd",
            "pk3cmd -P PIC16F628A -M -F",
            "--target",
            "pic16f628a",
            "-o",
            "build/blink.hex",
        ])
        .output()
        .expect("run print program command");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("pk3cmd -P PIC16F628A -M -F"));
    assert!(stdout.contains("build/blink.hex"));
}

#[test]
/// Verifies Phase 26 `--program` requires an explicit external command.
fn phase26_program_requires_command() {
    let out_hex = temp_file("phase26-program.hex");
    let output = Command::new(picc_bin())
        .current_dir(repo("."))
        .args([
            "--target",
            "pic16f628a",
            "-Wall",
            "-Wextra",
            "-O2",
            "-I",
            "include",
            "--program",
            "-o",
        ])
        .arg(&out_hex)
        .arg("examples/pic16f628a/blink.c")
        .output()
        .expect("run picc program");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("programmer command requested but missing"));
}

#[test]
/// Verifies Phase 26 hardware smoke examples build and expose configurable flashing.
fn phase26_hardware_smoke_examples_compile() {
    let examples = [
        (
            "pic16f628a",
            "examples/hardware/pic16f628a_led_blink/main.c",
            "examples/hardware/pic16f628a_led_blink/Makefile",
        ),
        (
            "pic16f877a",
            "examples/hardware/pic16f877a_led_blink/main.c",
            "examples/hardware/pic16f877a_led_blink/Makefile",
        ),
        (
            "pic16f877a",
            "examples/hardware/pic16f877a_timer_interrupt/main.c",
            "examples/hardware/pic16f877a_timer_interrupt/Makefile",
        ),
    ];

    for (target, source, makefile) in examples {
        let output =
            compile_example_via_picc_cli_with_extra_args(target, source, &["--verify-hex"]);
        assert_hex_is_programmable(&output);
        assert_makefile_shape(makefile);
    }
}

#[test]
/// Verifies Phase 27 float declarations, helpers, reports, and artifacts compile.
fn phase27_float_helpers_and_reports_compile() {
    let input = temp_file("phase27-float-report.c");
    fs::write(
        &input,
        r#"
float a = 1.5f;
float b = 2.0f;
float result;

void main(void) {
    result = a + b;
}
"#,
    )
    .expect("fixture");
    let output = temp_file("phase27-float-report.hex");
    let report = temp_file("phase27-float-report.mem");
    execute(CliOptions {
        command: CliCommand::Compile(CompileCommand {
            target: "pic16f877a".to_string(),
            input,
            output: output.clone(),
            include_dirs: vec![repo("include")],
            defines: BTreeMap::new(),
            optimization: OptimizationLevel::O2,
            artifacts: OutputArtifacts {
                map: true,
                list_file: true,
                memory_report: true,
                memory_report_file: Some(report.clone()),
                ..OutputArtifacts::default()
            },
            verbose: false,
            opt_report: false,
            stack_check: true,
            stack_report: true,
            stack_report_file: None,
            warning_profile: WarningProfile::default(),
        }),
    })
    .expect("compile float fixture");

    let map = fs::read_to_string(output.with_extension("map")).expect("map");
    let lst = fs::read_to_string(output.with_extension("lst")).expect("lst");
    let memory_report = fs::read_to_string(report).expect("memory report");
    assert!(map.contains("__rt_f32_add"));
    assert!(lst.contains("__rt_f32_add"));
    assert!(memory_report.contains("float helper"));
    assert!(memory_report.contains("__rt_f32_add"));
}

#[test]
/// Verifies malformed or unsupported Phase 27 float forms diagnose clearly.
fn phase27_float_diagnostics() {
    let bad_suffix = compile_error(
        "pic16f877a",
        "phase27-bad-float-suffix.c",
        "float result = 1.0d;",
    );
    assert!(bad_suffix.contains("unsupported float literal suffix"));

    let mixed = compile_error(
        "pic16f877a",
        "phase27-mixed-float.c",
        r#"
float a;
unsigned int b;
float result;
void main(void) {
    result = a + b;
}
"#,
    );
    assert!(mixed.contains("requires both operands to be `float`"));

    let div_zero = compile_error(
        "pic16f877a",
        "phase27-float-div-zero.c",
        r#"
float result;
void main(void) {
    result = 1.0f / 0.0f;
}
"#,
    );
    assert!(div_zero.contains("float division by constant zero"));

    let bitwise = compile_error(
        "pic16f877a",
        "phase27-float-bitwise.c",
        r#"
float a;
float b;
unsigned long result;
void main(void) {
    result = a & b;
}
"#,
    );
    assert!(bitwise.contains("is not supported for float operands"));
}

#[test]
/// Verifies float helper-backed work remains rejected inside interrupt handlers.
fn phase27_rejects_float_helper_inside_isr() {
    let error = compile_error(
        "pic16f877a",
        "phase27-float-isr.c",
        r#"
float a;
float b;
float result;
void __interrupt isr(void) {
    result = a + b;
}
void main(void) {}
"#,
    );

    assert!(error.contains("cannot use `Add` when it would lower through a runtime helper"));
}

#[test]
/// Verifies Phase 30 accepts simple ROM float tables.
fn phase30_compiles_rom_float_table_declaration() {
    let output = compile_source(
        "pic16f877a",
        "phase30-rom-float-decl.c",
        r#"
const __rom float table[] = { 1.0f, 2.0f };
void main(void) {}
"#,
    );

    let map = read_artifact(&output, "map");
    assert!(map.contains("table [rom, const"));
    assert_hex_is_programmable(&output);
}

#[test]
/// Verifies checked-in Phase 27 float examples compile cleanly on PIC16F877A.
fn phase27_float_examples_compile_via_picc() {
    for example in [
        "examples/pic16f877a/float_basic.c",
        "examples/pic16f877a/float_arithmetic.c",
        "examples/pic16f877a/float_casts.c",
        "examples/pic16f877a/float_struct.c",
        "examples/pic16f877a/float_sensor_scale.c",
    ] {
        let output = compile_example_via_picc_cli_with_extra_args(
            "pic16f877a",
            example,
            &["--size", "--verify-hex"],
        );
        assert_hex_is_programmable(&output);
    }
}

#[test]
/// Verifies Phase 28 folded float comparisons through the user-facing simulator workflow.
fn phase28_folded_float_comparison_runs_with_pic16_sim() {
    let input = temp_file("phase28-float-comparison.c");
    fs::write(
        &input,
        r#"
unsigned char same_flag;

void main(void) {
    same_flag = (1.5f == 1.5f);
}
"#,
    )
    .expect("fixture");
    let output = temp_file("phase28-float-comparison.hex");
    let compile = Command::new(picc_bin())
        .arg("--target")
        .arg("pic16f877a")
        .arg("-O2")
        .arg("-I")
        .arg(repo("include"))
        .arg("--map")
        .arg("-o")
        .arg(&output)
        .arg(&input)
        .output()
        .expect("run picc");
    assert!(
        compile.status.success(),
        "picc failed: {}",
        String::from_utf8_lossy(&compile.stderr)
    );

    let sim = Command::new(pic16_sim_bin())
        .arg(&output)
        .arg("--map")
        .arg(output.with_extension("map"))
        .arg("--run-until")
        .arg("__halt")
        .arg("--print-symbol")
        .arg("same_flag")
        .arg("--max-steps")
        .arg("200000")
        .output()
        .expect("run pic16-sim");
    assert!(
        sim.status.success(),
        "pic16-sim failed: {}",
        String::from_utf8_lossy(&sim.stderr)
    );
    assert!(String::from_utf8_lossy(&sim.stdout).contains("same_flag = 1"));
}

#[test]
/// Verifies Phase 28 cast helpers appear in memory and stack reports.
fn phase28_float_cast_helpers_are_reported() {
    let input = temp_file("phase28-float-cast-report.c");
    fs::write(
        &input,
        r#"
int source;
float value;
int result;

void main(void) {
    source = -3;
    value = (float)source;
    result = (int)value;
}
"#,
    )
    .expect("fixture");
    let output = temp_file("phase28-float-cast-report.hex");
    let memory_report = temp_file("phase28-float-cast-report.mem");
    let stack_report = temp_file("phase28-float-cast-report.stack");
    execute(CliOptions {
        command: CliCommand::Compile(CompileCommand {
            target: "pic16f877a".to_string(),
            input,
            output: output.clone(),
            include_dirs: vec![repo("include")],
            defines: BTreeMap::new(),
            optimization: OptimizationLevel::O2,
            artifacts: OutputArtifacts {
                map: true,
                list_file: true,
                memory_report: true,
                memory_report_file: Some(memory_report.clone()),
                ..OutputArtifacts::default()
            },
            verbose: false,
            opt_report: false,
            stack_check: true,
            stack_report: false,
            stack_report_file: Some(stack_report.clone()),
            warning_profile: WarningProfile::default(),
        }),
    })
    .expect("compile phase28 cast report");

    let map = fs::read_to_string(output.with_extension("map")).expect("map");
    let lst = fs::read_to_string(output.with_extension("lst")).expect("lst");
    let memory = fs::read_to_string(memory_report).expect("memory report");
    let stack = fs::read_to_string(stack_report).expect("stack report");
    assert!(map.contains("__rt_i32_to_f32"));
    assert!(map.contains("__rt_f32_to_i32"));
    assert!(lst.contains("__rt_i32_to_f32"));
    assert!(lst.contains("__rt_f32_to_i32"));
    assert!(memory.contains("float helper"));
    assert!(memory.contains("__rt_i32_to_f32"));
    assert!(memory.contains("__rt_f32_to_i32"));
    assert!(stack.contains("helper_extra="));
}

#[test]
/// Verifies Phase 28 unsupported float forms diagnose instead of silently lowering.
fn phase28_float_hardening_diagnostics() {
    let float_switch = compile_error(
        "pic16f877a",
        "phase28-float-switch.c",
        r#"
float selector;
void main(void) {
    switch (selector) {
    default:
        break;
    }
}
"#,
    );
    assert!(float_switch.contains("switch expression must have integer or enum type"));
}

#[test]
/// Verifies Phase 29 float compare and 32-bit conversion helpers appear in artifacts/reports.
fn phase29_float_helpers_are_reported() {
    let input = temp_file("phase29-float-compare-report.c");
    fs::write(
        &input,
        r#"
float a;
float b;
unsigned char result;

void main(void) {
    a = 1.5f;
    b = 1.0f;
    if (a > b) {
        result = 1;
    }
}
"#,
    )
    .expect("fixture");
    let output = temp_file("phase29-float-compare-report.hex");
    let memory_report = temp_file("phase29-float-compare-report.mem");
    let stack_report = temp_file("phase29-float-compare-report.stack");
    execute(CliOptions {
        command: CliCommand::Compile(CompileCommand {
            target: "pic16f877a".to_string(),
            input,
            output: output.clone(),
            include_dirs: vec![repo("include")],
            defines: BTreeMap::new(),
            optimization: OptimizationLevel::O2,
            artifacts: OutputArtifacts {
                map: true,
                list_file: true,
                memory_report: true,
                memory_report_file: Some(memory_report.clone()),
                ..OutputArtifacts::default()
            },
            verbose: false,
            opt_report: false,
            stack_check: true,
            stack_report: false,
            stack_report_file: Some(stack_report.clone()),
            warning_profile: WarningProfile::default(),
        }),
    })
    .expect("compile phase29 helper report");

    let map = fs::read_to_string(output.with_extension("map")).expect("map");
    let lst = fs::read_to_string(output.with_extension("lst")).expect("lst");
    let memory = fs::read_to_string(memory_report).expect("memory report");
    let stack = fs::read_to_string(stack_report).expect("stack report");
    assert!(map.contains("__rt_f32_cmp"));
    assert!(lst.contains("__rt_f32_cmp"));
    assert!(memory.contains("__rt_f32_cmp"));
    assert!(memory.contains("float helper"));
    assert!(stack.contains("helper_extra="));

    let input = temp_file("phase29-float-cast-report.c");
    fs::write(
        &input,
        r#"
long source;
float value;
long result;

void main(void) {
    source = 100000L;
    value = (float)source;
    result = (long)value;
}
"#,
    )
    .expect("fixture");
    let output = temp_file("phase29-float-cast-report.hex");
    let memory_report = temp_file("phase29-float-cast-report.mem");
    execute(CliOptions {
        command: CliCommand::Compile(CompileCommand {
            target: "pic16f877a".to_string(),
            input,
            output: output.clone(),
            include_dirs: vec![repo("include")],
            defines: BTreeMap::new(),
            optimization: OptimizationLevel::O2,
            artifacts: OutputArtifacts {
                map: true,
                list_file: true,
                memory_report: true,
                memory_report_file: Some(memory_report.clone()),
                ..OutputArtifacts::default()
            },
            verbose: false,
            opt_report: false,
            stack_check: true,
            stack_report: false,
            stack_report_file: None,
            warning_profile: WarningProfile::default(),
        }),
    })
    .expect("compile phase29 cast helper report");
    let map = fs::read_to_string(output.with_extension("map")).expect("map");
    let lst = fs::read_to_string(output.with_extension("lst")).expect("lst");
    let memory = fs::read_to_string(memory_report).expect("memory report");
    for helper in ["__rt_i32_to_f32", "__rt_f32_to_i32"] {
        assert!(map.contains(helper), "missing {helper} in map");
        assert!(lst.contains(helper), "missing {helper} in listing");
        assert!(memory.contains(helper), "missing {helper} in memory report");
    }
}

#[test]
/// Verifies Phase 29 float diagnostics remain explicit.
fn phase29_float_completion_diagnostics() {
    let isr_compare = compile_error(
        "pic16f877a",
        "phase29-float-compare-isr.c",
        r#"
float a;
float b;
void __interrupt isr(void) {
    if (a > b) {
    }
}
void main(void) {}
"#,
    );
    assert!(
        isr_compare.contains("cannot use `Greater` when it would lower through a runtime helper")
    );

    let negative_unsigned = compile_error(
        "pic16f877a",
        "phase29-negative-float-to-ulong.c",
        r#"
unsigned long result;
void main(void) {
    result = (unsigned long)-1.0f;
}
"#,
    );
    assert!(negative_unsigned.contains("constant float-to-unsigned-long cast is out of range"));

    let out_of_range = compile_error(
        "pic16f877a",
        "phase29-float-to-long-range.c",
        r#"
long result;
void main(void) {
    result = (long)3000000000.0f;
}
"#,
    );
    assert!(out_of_range.contains("constant float-to-long cast is out of range"));
}

#[test]
/// Verifies checked-in Phase 28 float examples compile cleanly on PIC16F877A.
fn phase28_float_examples_compile_via_picc() {
    for example in [
        "examples/pic16f877a/float_dynamic_casts.c",
        "examples/pic16f877a/float_comparisons.c",
        "examples/pic16f877a/float_resource_report.c",
        "examples/pic16f877a/float_fixed_interop.c",
        "examples/pic16f877a/float_limitations.c",
    ] {
        let output = compile_example_via_picc_cli_with_extra_args(
            "pic16f877a",
            example,
            &["--size", "--verify-hex"],
        );
        assert_hex_is_programmable(&output);
    }
}

#[test]
/// Verifies checked-in Phase 29 float examples compile cleanly on PIC16F877A.
fn phase29_float_examples_compile_via_picc() {
    for example in [
        "examples/pic16f877a/float_dynamic_comparisons.c",
        "examples/pic16f877a/float_long_casts.c",
        "examples/pic16f877a/float_threshold_control.c",
        "examples/pic16f877a/float_loop_compare.c",
    ] {
        let output = compile_example_via_picc_cli_with_extra_args(
            "pic16f877a",
            example,
            &["--size", "--verify-hex"],
        );
        assert_hex_is_programmable(&output);
    }
}

#[test]
/// Verifies Phase 30 ROM float tables emit little-endian f32 bytes and resource metadata.
fn phase30_rom_float_tables_emit_and_report() {
    let input = temp_file("phase30-rom-float-report.c");
    fs::write(
        &input,
        r#"
const __rom float calibration[] = { 1.0f, 1.5f, 2.0f };
float value;

void main(void) {
    value = calibration[1];
}
"#,
    )
    .expect("fixture");
    let output = temp_file("phase30-rom-float-report.hex");
    let memory_report = temp_file("phase30-rom-float-report.mem");
    execute(CliOptions {
        command: CliCommand::Compile(CompileCommand {
            target: "pic16f877a".to_string(),
            input,
            output: output.clone(),
            include_dirs: vec![repo("include")],
            defines: BTreeMap::new(),
            optimization: OptimizationLevel::O2,
            artifacts: OutputArtifacts {
                map: true,
                list_file: true,
                memory_report: true,
                memory_report_file: Some(memory_report.clone()),
                ..OutputArtifacts::default()
            },
            verbose: false,
            opt_report: false,
            stack_check: false,
            stack_report: false,
            stack_report_file: None,
            warning_profile: WarningProfile::default(),
        }),
    })
    .expect("compile phase30 ROM float report");

    let map = fs::read_to_string(output.with_extension("map")).expect("map");
    let lst = fs::read_to_string(output.with_extension("lst")).expect("listing");
    let memory = fs::read_to_string(memory_report).expect("memory report");
    let rom_addr = map_symbol_address(&map, "calibration [rom, const").expect("rom symbol");
    let hex = read_hex_bytes(&output);

    assert!(map.contains("calibration [rom, const, 3 element(s), 12 byte(s)]"));
    assert!(lst.contains("ROM calibration [rom, const"));
    assert!(memory.contains("calibration [rom, const"));
    assert!(memory.contains("ROM RETLW table"));
    assert!(map.contains("ROM table words"));
    assert_eq!(hex.get(&(rom_addr * 2 + 2)).copied(), Some(0x00));
    assert_eq!(hex.get(&(rom_addr * 2 + 3)).copied(), Some(0x34));
    assert_eq!(hex.get(&(rom_addr * 2 + 4)).copied(), Some(0x00));
    assert_eq!(hex.get(&(rom_addr * 2 + 5)).copied(), Some(0x34));
    assert_eq!(hex.get(&(rom_addr * 2 + 6)).copied(), Some(0x80));
    assert_eq!(hex.get(&(rom_addr * 2 + 7)).copied(), Some(0x34));
    assert_eq!(hex.get(&(rom_addr * 2 + 8)).copied(), Some(0x3F));
    assert_eq!(hex.get(&(rom_addr * 2 + 9)).copied(), Some(0x34));
}

#[test]
/// Verifies Phase 30 keeps unsupported ROM float forms explicit.
fn phase30_rom_float_diagnostics() {
    let nonconst = compile_error(
        "pic16f877a",
        "phase30-rom-float-nonconst.c",
        r#"
__rom float table[] = { 1.0f };
void main(void) {}
"#,
    );
    assert!(nonconst.contains("program-memory object `table` must be declared `const`"));

    let local = compile_error(
        "pic16f877a",
        "phase30-rom-float-local.c",
        r#"
void main(void) {
    const __rom float table[] = { 1.0f };
}
"#,
    );
    assert!(local.contains("local `table` cannot use `__rom` storage"));

    let pointer_mix = compile_error(
        "pic16f877a",
        "phase30-rom-float-pointer.c",
        r#"
const __rom float table[] = { 1.0f };
float *p;
void main(void) {
    p = table;
}
"#,
    );
    assert!(pointer_mix.contains("program-memory arrays do not decay to data-space pointers"));

    let address = compile_error(
        "pic16f877a",
        "phase30-rom-float-address.c",
        r#"
const __rom float table[] = { 1.0f };
float *p;
void main(void) {
    p = &table[0];
}
"#,
    );
    assert!(
        address.contains("taking the address of a program-memory array element is not supported")
    );

    let write = compile_error(
        "pic16f877a",
        "phase30-rom-float-write.c",
        r#"
const __rom float table[] = { 1.0f };
void main(void) {
    table[0] = 2.0f;
}
"#,
    );
    assert!(write.contains("writing to a program-memory array element is not allowed"));
}

#[test]
/// Verifies dynamic ROM float reads remain rejected inside interrupt handlers.
fn phase30_rejects_dynamic_rom_float_read_in_isr() {
    let error = compile_error(
        "pic16f877a",
        "phase30-rom-float-isr.c",
        r#"
const __rom float table[] = { 1.0f, 2.0f };
unsigned char index;
float value;

void __interrupt isr(void) {
    value = table[index];
}

void main(void) {}
"#,
    );

    assert!(error.contains("cannot perform dynamic ROM reads"));
}

#[test]
/// Verifies checked-in Phase 30 ROM float examples compile cleanly on PIC16F877A.
fn phase30_float_rom_examples_compile_via_picc() {
    for example in [
        "examples/pic16f877a/float_rom_table.c",
        "examples/pic16f877a/float_calibration_table.c",
        "examples/pic16f877a/float_rom_threshold.c",
        "examples/pic16f877a/float_static_init.c",
    ] {
        let output = compile_example_via_picc_cli_with_extra_args(
            "pic16f877a",
            example,
            &["--size", "--verify-hex"],
        );
        assert_hex_is_programmable(&output);
    }
}

#[test]
/// Verifies Phase 36 math helpers are reported only when used.
fn phase36_math_helpers_are_reported_and_pruned() {
    let math_source = r#"
#include <math.h>

float input;
float output;

void main(void) {
    input = -1.75f;
    output = floorf(input);
}
"#;
    let (_hex, stdout, memory_report) =
        compile_profile_source_size_report("balanced", "phase36-math-report", math_source, &[]);
    assert!(stdout.contains("math:"));
    assert!(memory_report.contains("__rt_f32_floor"));
    assert!(memory_report.contains("math helper"));

    let unused_source = r#"
#include <math.h>

float value;

void main(void) {
    value = 1.5f;
}
"#;
    let output = compile_source("pic16f877a", "phase36-unused-math.c", unused_source);
    let map = read_artifact(&output, "map");
    assert!(!map.contains("__rt_f32_floor"));
    assert!(!map.contains("__rt_f32_trunc"));
    assert!(!map.contains("__rt_f32_round"));
}

#[test]
/// Verifies Phase 36 constant folding handles supported math calls.
fn phase36_constant_folds_supported_math_calls() {
    let output = compile_source(
        "pic16f877a",
        "phase36-folded-math.c",
        r#"
#include <math.h>

float a = fabsf(-1.5f);
float b = truncf(-1.75f);
float c = floorf(-1.25f);
float d = ceilf(-1.75f);
float e = roundf(-1.5f);

void main(void) {}
"#,
    );
    let map = read_artifact(&output, "map");
    assert!(!map.contains("__rt_f32_fabs"));
    assert!(!map.contains("__rt_f32_floor"));
    assert_hex_is_programmable(&output);
}

#[test]
/// Verifies Phase 36 ISR policy allows inline `fabsf` but rejects helper-backed math.
fn phase36_math_isr_policy_is_explicit() {
    let output = compile_source(
        "pic16f877a",
        "phase36-isr-fabs.c",
        r#"
#include <math.h>

float value;

void __interrupt isr(void) {
    value = fabsf(value);
}

void main(void) {}
"#,
    );
    assert_hex_is_programmable(&output);

    let error = compile_error(
        "pic16f877a",
        "phase36-isr-floor.c",
        r#"
#include <math.h>

float value;

void __interrupt isr(void) {
    value = floorf(value);
}

void main(void) {}
"#,
    );
    assert!(error.contains("float math helper"));
}

#[test]
/// Verifies checked-in Phase 36 math examples compile cleanly on PIC16F877A.
fn phase36_math_examples_compile_via_picc() {
    for example in [
        "examples/pic16f877a/math_fabs_trunc.c",
        "examples/pic16f877a/math_floor_ceil.c",
        "examples/pic16f877a/math_round.c",
        "examples/pic16f877a/math_rom_calibration.c",
        "examples/pic16f877a/math_resource_report.c",
    ] {
        let output = compile_example_via_picc_cli_with_extra_args(
            "pic16f877a",
            example,
            &["--size", "--verify-hex"],
        );
        assert_hex_is_programmable(&output);
    }
}

#[test]
/// Verifies Phase 37 sqrtf is reported, pruned, and constant-folded.
fn phase37_sqrtf_reporting_pruning_and_folding() {
    let sqrt_source = r#"
#include <math.h>

float input;
float output;

void main(void) {
    input = 2.25f;
    output = sqrtf(input);
}
"#;
    let (hex, stdout, memory_report) =
        compile_profile_source_size_report("balanced", "phase37-sqrt-report", sqrt_source, &[]);
    assert!(stdout.contains("math:"));
    assert!(memory_report.contains("__rt_f32_sqrt"));
    assert!(memory_report.contains("math helper"));
    let map = read_artifact(&hex, "map");
    let listing = read_artifact(&hex, "lst");
    assert!(map.contains("__rt_f32_sqrt"));
    assert!(listing.contains("__rt_f32_sqrt"));

    let folded = compile_source(
        "pic16f877a",
        "phase37-folded-sqrt.c",
        r#"
#include <math.h>

float a = sqrtf(4.0f);
float b = sqrtf(-1.0f);

void main(void) {}
"#,
    );
    let folded_map = read_artifact(&folded, "map");
    assert!(!folded_map.contains("__rt_f32_sqrt"));

    let unused = compile_source(
        "pic16f877a",
        "phase37-unused-sqrt.c",
        r#"
#include <math.h>

float value;

void main(void) {
    value = 1.5f;
}
"#,
    );
    let unused_map = read_artifact(&unused, "map");
    assert!(!unused_map.contains("__rt_f32_sqrt"));
}

#[test]
/// Verifies unsupported sqrt spellings and invalid sqrtf calls are diagnostics, not implicit remaps.
fn phase37_sqrtf_diagnostics_are_explicit() {
    let wrong_count = compile_error(
        "pic16f877a",
        "phase37-sqrtf-wrong-count.c",
        r#"
#include <math.h>

float value;

void main(void) {
    value = sqrtf();
}
"#,
    );
    assert!(wrong_count.contains("expects 1 argument"));

    let wrong_type = compile_error(
        "pic16f877a",
        "phase37-sqrtf-wrong-type.c",
        r#"
#include <math.h>

float value;

void main(void) {
    int input = 4;
    value = sqrtf(input);
}
"#,
    );
    assert!(wrong_type.contains("expects a float argument"));

    let unsupported_sqrt = compile_error(
        "pic16f877a",
        "phase37-sqrt-unsupported.c",
        r#"
#include <math.h>

float value;

void main(void) {
    value = sqrt(4.0f);
}
"#,
    );
    assert!(unsupported_sqrt.contains("sqrt"));
}

#[test]
/// Verifies Phase 37 keeps helper-backed sqrtf out of ISRs.
fn phase37_sqrtf_isr_policy_is_explicit() {
    let error = compile_error(
        "pic16f877a",
        "phase37-isr-sqrt.c",
        r#"
#include <math.h>

float value;

void __interrupt isr(void) {
    value = sqrtf(value);
}

void main(void) {}
"#,
    );
    assert!(error.contains("float math helper"));
}

#[test]
/// Verifies checked-in Phase 37 sqrtf examples compile cleanly on PIC16F877A.
fn phase37_sqrtf_examples_compile_via_picc() {
    for example in [
        "examples/pic16f877a/math_sqrt_basic.c",
        "examples/pic16f877a/math_sqrt_rom.c",
        "examples/pic16f877a/math_sqrt_resource_report.c",
    ] {
        let output = compile_example_via_picc_cli_with_extra_args(
            "pic16f877a",
            example,
            &["--size", "--verify-hex"],
        );
        assert_hex_is_programmable(&output);
    }
}

#[test]
/// Verifies Phase 38 reports math profile and tags compact sqrtf helper variants.
fn phase38_math_profile_reporting_tags_sqrtf_variant() {
    let sqrt_source = r#"
#include <math.h>

float input;
float output;

void main(void) {
    input = 3.0f;
    output = sqrtf(input);
}
"#;
    let (compact_hex, compact_stdout, compact_report) = compile_profile_source_size_report(
        "balanced",
        "phase38-sqrt-compact-report",
        sqrt_source,
        &["--math-profile", "compact"],
    );
    assert!(compact_stdout.contains("Math profile: compact"));
    assert!(compact_report.contains("math profile: compact"));
    assert!(compact_report.contains("__rt_f32_sqrt"));
    assert!(compact_report.contains("variant=compact_approx"));
    assert!(compact_report.contains("category=math"));
    assert!(
        String::from_utf8_lossy(
            &Command::new(picc_bin())
                .current_dir(repo("."))
                .args(["--help"])
                .output()
                .expect("picc help")
                .stdout
        )
        .contains("--math-profile")
    );
    let compact_map = read_artifact(&compact_hex, "map");
    let compact_listing = read_artifact(&compact_hex, "lst");
    assert!(compact_map.contains("Math profile: compact"));
    assert!(compact_listing.contains("variant=compact_approx"));

    let (balanced_hex, balanced_stdout, balanced_report) = compile_profile_source_size_report(
        "balanced",
        "phase38-sqrt-balanced-report",
        sqrt_source,
        &["--math-profile", "balanced"],
    );
    assert!(balanced_stdout.contains("Math profile: balanced"));
    assert!(balanced_report.contains("math profile: balanced"));
    assert!(balanced_report.contains("variant=compact_approx"));
    assert_hex_is_programmable(&compact_hex);
    assert_hex_is_programmable(&balanced_hex);
}

#[test]
/// Verifies Phase 39 precise profile folds constants and emits the refined dynamic helper.
fn phase39_precise_math_profile_emits_refined_sqrtf() {
    let folded = r#"
#include <math.h>

float result = sqrtf(4.0f);

void main(void) {}
"#;
    let (folded_hex, folded_stdout, folded_report) = compile_profile_source_size_report(
        "balanced",
        "phase38-precise-folded-sqrt",
        folded,
        &["--math-profile", "precise"],
    );
    assert!(folded_stdout.contains("Math profile: precise"));
    assert!(folded_report.contains("math profile: precise"));
    assert!(!read_artifact(&folded_hex, "map").contains("__rt_f32_sqrt"));
    assert_hex_is_programmable(&folded_hex);

    let dynamic = r#"
#include <math.h>

float input;
float output;

void main(void) {
    input = 3.0f;
    output = sqrtf(input);
}
"#;
    let (dynamic_hex, dynamic_stdout, dynamic_report) = compile_profile_source_size_report(
        "balanced",
        "phase39-precise-dynamic-sqrt",
        dynamic,
        &["--math-profile", "precise"],
    );
    assert!(dynamic_stdout.contains("Math profile: precise"));
    assert!(dynamic_report.contains("math profile: precise"));
    assert!(dynamic_report.contains("__rt_f32_sqrt"));
    assert!(dynamic_report.contains("variant=precise_table_refined"));
    assert_hex_is_programmable(&dynamic_hex);
}

#[test]
/// Verifies Phase 39 precise sqrtf costs more than compact and reports a different variant.
fn phase39_precise_sqrtf_is_larger_than_compact_and_reported() {
    let source = r#"
#include <math.h>

float input;
float output;

void main(void) {
    input = 3.0f;
    output = sqrtf(input);
}
"#;
    let (compact_hex, compact_stdout, compact_report) = compile_profile_source_size_report(
        "balanced",
        "phase39-compact-size",
        source,
        &["--math-profile", "compact"],
    );
    let (precise_hex, precise_stdout, precise_report) = compile_profile_source_size_report(
        "balanced",
        "phase39-precise-size",
        source,
        &["--math-profile", "precise"],
    );
    let compact_words = parse_program_words(&compact_stdout);
    let precise_words = parse_program_words(&precise_stdout);
    assert!(
        precise_words > compact_words,
        "precise sqrtf should cost more than compact: precise={precise_words} compact={compact_words}"
    );
    assert!(compact_report.contains("variant=compact_approx"));
    assert!(precise_report.contains("variant=precise_table_refined"));
    assert_hex_is_programmable(&compact_hex);
    assert_hex_is_programmable(&precise_hex);
}

#[test]
/// Verifies unsupported math profile names are rejected at CLI parsing.
fn phase38_rejects_unknown_math_profile() {
    let output = Command::new(picc_bin())
        .current_dir(repo("."))
        .args([
            "--target",
            "pic16f877a",
            "--math-profile",
            "ieee",
            "-o",
            "ignored.hex",
            "examples/pic16f877a/math_sqrt_basic.c",
        ])
        .output()
        .expect("run picc");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("unsupported math profile"));
}

#[test]
/// Verifies checked-in Phase 38 math-profile examples compile where currently supported.
fn phase38_math_profile_examples_compile_via_picc() {
    for example in [
        "examples/pic16f877a/math_sqrt_profile_compact.c",
        "examples/pic16f877a/math_profile_resource_compare.c",
    ] {
        let output = compile_example_via_picc_cli_with_extra_args(
            "pic16f877a",
            example,
            &[
                "--size",
                "--memory-report",
                "--verify-hex",
                "--math-profile",
                "compact",
            ],
        );
        assert_hex_is_programmable(&output);
    }

    let precise_output = compile_example_via_picc_cli_with_extra_args(
        "pic16f877a",
        "examples/pic16f877a/math_sqrt_profile_precise.c",
        &[
            "--size",
            "--memory-report",
            "--verify-hex",
            "--math-profile",
            "precise",
        ],
    );
    assert_hex_is_programmable(&precise_output);
}

#[test]
/// Verifies checked-in Phase 39 precise sqrtf examples compile and report resources.
fn phase39_precise_sqrtf_examples_compile_via_picc() {
    for example in [
        "examples/pic16f877a/math_sqrt_precise.c",
        "examples/pic16f877a/math_sqrt_profile_compare.c",
        "examples/pic16f877a/math_sqrt_precise_resource_report.c",
    ] {
        let output = compile_example_via_picc_cli_with_extra_args(
            "pic16f877a",
            example,
            &[
                "--size",
                "--memory-report",
                "--verify-hex",
                "--math-profile",
                "precise",
            ],
        );
        assert_hex_is_programmable(&output);
    }
}

#[test]
/// Verifies Phase 40 reports the finite sqrtf accuracy policy and keeps folded sqrtf helper-free.
fn phase40_precise_sqrtf_accuracy_policy_is_reported() {
    let dynamic = r#"
#include <math.h>

float input;
float output;

void main(void) {
    input = 5.0f;
    output = sqrtf(input);
}
"#;
    let (dynamic_hex, dynamic_stdout, dynamic_report) = compile_profile_source_size_report(
        "balanced",
        "phase40-precise-accuracy-report",
        dynamic,
        &["--math-profile", "precise"],
    );
    assert!(dynamic_stdout.contains("Math profile: precise"));
    assert!(dynamic_stdout.contains("Math accuracy: precise sqrtf"));
    assert!(dynamic_report.contains("math profile: precise"));
    assert!(dynamic_report.contains("math accuracy: precise sqrtf"));
    assert!(dynamic_report.contains("[0.25, 64.0]"));
    assert!(dynamic_report.contains("+/-0.03125"));
    assert!(dynamic_report.contains("variant=precise_table_refined"));
    assert_hex_is_programmable(&dynamic_hex);

    let folded = r#"
#include <math.h>

float output = sqrtf(5.0f);

void main(void) {}
"#;
    let (folded_hex, _folded_stdout, folded_report) = compile_profile_source_size_report(
        "balanced",
        "phase40-precise-folded-report",
        folded,
        &["--math-profile", "precise"],
    );
    assert!(!read_artifact(&folded_hex, "map").contains("__rt_f32_sqrt"));
    assert!(folded_report.contains("math accuracy: precise sqrtf"));
    assert_hex_is_programmable(&folded_hex);
}

#[test]
/// Verifies checked-in Phase 40 sqrt accuracy examples compile and report resources.
fn phase40_sqrtf_accuracy_examples_compile_via_picc() {
    for example in [
        "examples/pic16f877a/math_sqrt_accuracy.c",
        "examples/pic16f877a/math_sqrt_precise_table.c",
        "examples/pic16f877a/math_sqrt_compact_vs_precise.c",
    ] {
        let output = compile_example_via_picc_cli_with_extra_args(
            "pic16f877a",
            example,
            &[
                "--size",
                "--memory-report",
                "--verify-hex",
                "--math-profile",
                "precise",
            ],
        );
        assert_hex_is_programmable(&output);
    }
}

#[test]
/// Verifies Phase 41 folds constant min/max calls and prunes unused helpers.
fn phase41_minmax_folds_and_prunes_helpers() {
    let folded = r#"
#include <math.h>

float low = fminf(1.0f, 2.0f);
float high = fmaxf(-1.0f, -2.0f);

void main(void) {}
"#;
    let (folded_hex, _stdout, folded_report) =
        compile_profile_source_size_report("balanced", "phase41-minmax-folded", folded, &[]);
    let map = read_artifact(&folded_hex, "map");
    assert!(!map.contains("__rt_f32_min"));
    assert!(!map.contains("__rt_f32_max"));
    assert!(!map.contains("__rt_f32_cmp"));
    assert!(folded_report.contains("math profile: balanced"));
    assert_hex_is_programmable(&folded_hex);

    let unused = r#"
#include <math.h>

float result;

void main(void) {
    result = 1.0f;
}
"#;
    let (unused_hex, _stdout, _report) =
        compile_profile_source_size_report("balanced", "phase41-minmax-unused", unused, &[]);
    let map = read_artifact(&unused_hex, "map");
    assert!(!map.contains("__rt_f32_min"));
    assert!(!map.contains("__rt_f32_max"));
    assert_hex_is_programmable(&unused_hex);
}

#[test]
/// Verifies Phase 42 dynamic min/max lowers to compact compare/select without min/max helpers.
fn phase42_minmax_lowers_to_compact_compare_select() {
    let dynamic = r#"
#include <math.h>

float a;
float b;
float low;
float high;

void main(void) {
    a = 1.5f;
    b = 2.0f;
    low = fminf(a, b);
    high = fmaxf(a, b);
}
"#;
    let (dynamic_hex, stdout, report) =
        compile_profile_source_size_report("balanced", "phase41-minmax-report", dynamic, &[]);
    let map = read_artifact(&dynamic_hex, "map");
    assert!(stdout.contains("Math profile: balanced"));
    assert!(!map.contains("__rt_f32_min"));
    assert!(!map.contains("__rt_f32_max"));
    assert!(!report.contains("__rt_f32_min"));
    assert!(!report.contains("__rt_f32_max"));
    assert!(report.contains("__rt_f32_cmp"));
    assert!(report.contains("category=float"));
    assert!(parse_runtime_helper_actual_words(&report, "__rt_f32_cmp") < 1200);
    assert_hex_is_programmable(&dynamic_hex);
}

#[test]
/// Verifies Phase 42 compact finite f32 compare is reported below the old Q16 conversion cost.
fn phase42_float_compare_helper_cost_is_compact() {
    let source = r#"
float a;
float b;
unsigned char result;

void main(void) {
    a = -3.0f;
    b = -2.0f;
    if (a < b) {
        result = 1;
    } else {
        result = 0;
    }
}
"#;
    let (hex, stdout, report) =
        compile_profile_source_size_report("balanced", "phase42-float-compare-cost", source, &[]);
    assert!(stdout.contains("Runtime helpers:"));
    assert!(report.contains("__rt_f32_cmp"));
    assert!(parse_runtime_helper_actual_words(&report, "__rt_f32_cmp") < 1200);
    assert_hex_is_programmable(&hex);
}

#[test]
/// Verifies Phase 42 keeps min/max size profile sane under balanced and small runtime profiles.
fn phase42_minmax_runtime_profiles_do_not_emit_wrappers() {
    let source = r#"
#include <math.h>

float a;
float b;
float low;
float high;
unsigned char alarm;

void main(void) {
    a = 3.0f;
    b = 2.0f;
    low = fminf(a, b);
    high = fmaxf(a, b);
    if (high > low) {
        alarm = 1;
    }
}
"#;
    let (_balanced_hex, balanced_stdout, balanced_report) =
        compile_profile_source_size_report("balanced", "phase42-minmax-balanced", source, &[]);
    let (small_hex, small_stdout, small_report) =
        compile_profile_source_size_report("small", "phase42-minmax-small", source, &[]);
    assert!(parse_program_words(&small_stdout) <= parse_program_words(&balanced_stdout));
    for report in [&balanced_report, &small_report] {
        assert!(report.contains("__rt_f32_cmp"));
        assert!(!report.contains("__rt_f32_min"));
        assert!(!report.contains("__rt_f32_max"));
        assert!(parse_runtime_helper_actual_words(report, "__rt_f32_cmp") < 1200);
    }
    assert_hex_is_programmable(&small_hex);
}

#[test]
/// Verifies Phase 41 min/max diagnostics are explicit and no double forms are remapped.
fn phase41_minmax_diagnostics_are_explicit() {
    let wrong_count = compile_error(
        "pic16f877a",
        "phase41-fminf-wrong-count.c",
        r#"
#include <math.h>

float value;

void main(void) {
    value = fminf(1.0f);
}
"#,
    );
    assert!(wrong_count.contains("expects 2 argument"));

    let wrong_type = compile_error(
        "pic16f877a",
        "phase41-fmaxf-wrong-type.c",
        r#"
#include <math.h>

float value;

void main(void) {
    int input = 4;
    value = fmaxf(input, 2.0f);
}
"#,
    );
    assert!(wrong_type.contains("expects float arguments"));

    let unsupported_fmin = compile_error(
        "pic16f877a",
        "phase41-fmin-unsupported.c",
        r#"
#include <math.h>

float value;

void main(void) {
    value = fmin(1.0f, 2.0f);
}
"#,
    );
    assert!(unsupported_fmin.contains("fmin"));
}

#[test]
/// Verifies Phase 41 keeps helper-backed min/max out of ISRs.
fn phase41_minmax_isr_policy_is_explicit() {
    let stderr = compile_error(
        "pic16f877a",
        "phase41-fminf-isr.c",
        r#"
#include <math.h>

float value;

void __interrupt isr(void) {
    value = fminf(value, 1.0f);
}

void main(void) {}
"#,
    );
    assert!(stderr.contains("cannot call `fminf`"));
    assert!(stderr.contains("float math helper"));
}

#[test]
/// Verifies checked-in Phase 41 min/max examples compile and report resources.
fn phase41_minmax_examples_compile_via_picc() {
    for example in [
        "examples/pic16f877a/math_minmax_basic.c",
        "examples/pic16f877a/math_minmax_rom.c",
        "examples/pic16f877a/math_minmax_threshold.c",
        "examples/pic16f877a/math_minmax_resource_report.c",
    ] {
        let output = compile_example_via_picc_cli_with_extra_args(
            "pic16f877a",
            example,
            &["--size", "--memory-report", "--verify-hex"],
        );
        assert_hex_is_programmable(&output);
    }
}

#[test]
/// Verifies checked-in Phase 42 compare/minmax compaction examples compile and report resources.
fn phase42_compare_minmax_examples_compile_via_picc() {
    for example in [
        "examples/pic16f877a/float_compare_runtime.c",
        "examples/pic16f877a/math_minmax_compact.c",
        "examples/pic16f877a/math_minmax_resource_compare.c",
    ] {
        let output = compile_example_via_picc_cli_with_extra_args(
            "pic16f877a",
            example,
            &["--size", "--memory-report", "--verify-hex"],
        );
        assert_hex_is_programmable(&output);
    }
}

#[test]
/// Verifies Phase 43 reports finite sin/cos helpers and internal ROM math tables.
fn phase43_sincos_reporting_and_pruning() {
    let source = r#"
#include <math.h>

float angle;
float s;
float c;

void main(void) {
    angle = 1.5707963f;
    s = sinf(angle);
    c = cosf(angle);
}
"#;
    let (hex, stdout, report) = compile_profile_source_size_report(
        "balanced",
        "phase43-sincos-report",
        source,
        &["--math-profile", "balanced"],
    );
    let map = read_artifact(&hex, "map");
    let listing = read_artifact(&hex, "lst");
    assert!(stdout.contains("Math profile: balanced"));
    assert!(stdout.contains("validated range [-2pi,+2pi]"));
    assert!(report.contains("__rt_f32_sin"));
    assert!(report.contains("__rt_f32_cos"));
    assert!(report.contains("__rt_f32_sincos_core"));
    assert!(report.contains("variant=wrapper"));
    assert!(report.contains("variant=shared_core_balanced"));
    assert!(report.contains("validated range [-2pi,+2pi]"));
    assert!(report.contains("scoped moderate aliases through +/-8pi"));
    assert!(report.contains("__rt_f32_sin -> __rt_f32_sincos_core"));
    assert!(report.contains("__rt_f32_cos -> __rt_f32_sincos_core"));
    assert!(report.contains("__rt_math_sin_qwave_table_balanced"));
    assert!(map.contains("__rt_math_sin_qwave_table_balanced"));
    assert!(map.contains("__rt_f32_sincos_core"));
    assert!(listing.contains("__rt_math_sin_qwave_table_balanced"));
    assert!(listing.contains("variant=shared_core_balanced"));
    assert_hex_is_programmable(&hex);

    let folded = r#"
#include <math.h>

float s = sinf(0.0f);
float c = cosf(0.0f);

void main(void) {}
"#;
    let (folded_hex, _folded_stdout, _folded_report) =
        compile_profile_source_size_report("balanced", "phase43-sincos-folded", folded, &[]);
    let folded_map = read_artifact(&folded_hex, "map");
    assert!(!folded_map.contains("__rt_f32_sin"));
    assert!(!folded_map.contains("__rt_f32_cos"));
    assert!(!folded_map.contains("__rt_f32_sincos_core"));
    assert!(!folded_map.contains("__rt_math_sin_qwave_table"));

    let unused = r#"
#include <math.h>

float value;

void main(void) {
    value = 1.0f;
}
"#;
    let (unused_hex, _unused_stdout, _unused_report) =
        compile_profile_source_size_report("balanced", "phase43-sincos-unused", unused, &[]);
    let unused_map = read_artifact(&unused_hex, "map");
    assert!(!unused_map.contains("__rt_f32_sin"));
    assert!(!unused_map.contains("__rt_f32_cos"));
    assert!(!unused_map.contains("__rt_f32_sincos_core"));
    assert!(!unused_map.contains("__rt_math_sin_qwave_table"));
}

#[test]
/// Verifies Phase 45 constant folded trig calls stay host-folded and do not emit trig runtime.
fn phase45_sincos_constant_folding_prunes_trig_runtime() {
    let source = r#"
#include <math.h>

float s = sinf(0.78539816f);
float c = cosf(0.78539816f);

void main(void) {}
"#;
    let (hex, _stdout, report) =
        compile_profile_source_size_report("balanced", "phase45-sincos-fold-pi4", source, &[]);
    let map = read_artifact(&hex, "map");
    assert!(!map.contains("__rt_f32_sin"));
    assert!(!map.contains("__rt_f32_cos"));
    assert!(!map.contains("__rt_f32_sincos_core"));
    assert!(!map.contains("__rt_math_sin_qwave_table"));
    assert!(!report.contains("__rt_f32_sincos_core"));
}

#[test]
/// Verifies Phase 44 emits one shared trig core and prunes unused sin/cos wrappers.
fn phase44_sincos_shared_core_pruning_and_size() {
    let sin_only = r#"
#include <math.h>

float angle;
float value;

void main(void) {
    angle = 1.5707963f;
    value = sinf(angle);
}
"#;
    let (sin_hex, _sin_stdout, sin_report) = compile_profile_source_size_report(
        "balanced",
        "phase44-sin-only",
        sin_only,
        &["--math-profile", "balanced"],
    );
    let sin_map = read_artifact(&sin_hex, "map");
    assert!(rendered_map_has_symbol(&sin_map, "__rt_f32_sin"));
    assert!(rendered_map_has_symbol(&sin_map, "__rt_f32_sincos_core"));
    assert!(!rendered_map_has_symbol(&sin_map, "__rt_f32_cos"));
    assert!(sin_report.contains("__rt_f32_sin -> __rt_f32_sincos_core"));
    assert!(sin_report.contains("variant=shared_core_balanced"));

    let cos_only = r#"
#include <math.h>

float angle;
float value;

void main(void) {
    angle = 3.1415927f;
    value = cosf(angle);
}
"#;
    let (cos_hex, _cos_stdout, cos_report) = compile_profile_source_size_report(
        "balanced",
        "phase44-cos-only",
        cos_only,
        &["--math-profile", "balanced"],
    );
    let cos_map = read_artifact(&cos_hex, "map");
    assert!(rendered_map_has_symbol(&cos_map, "__rt_f32_cos"));
    assert!(rendered_map_has_symbol(&cos_map, "__rt_f32_sincos_core"));
    assert!(!rendered_map_has_symbol(&cos_map, "__rt_f32_sin"));
    assert!(cos_report.contains("__rt_f32_cos -> __rt_f32_sincos_core"));

    let both = r#"
#include <math.h>

float angle;
float s;
float c;

void main(void) {
    angle = 1.5707963f;
    s = sinf(angle);
    c = cosf(angle);
}
"#;
    let (_both_hex, _both_stdout, both_report) = compile_profile_source_size_report(
        "balanced",
        "phase44-sincos-both",
        both,
        &["--math-profile", "balanced"],
    );
    let sin_words = parse_runtime_helper_actual_words(&both_report, "__rt_f32_sin");
    let cos_words = parse_runtime_helper_actual_words(&both_report, "__rt_f32_cos");
    let core_words = parse_runtime_helper_actual_words(&both_report, "__rt_f32_sincos_core");
    assert!(sin_words < 700, "sin wrapper too large: {sin_words}");
    assert!(cos_words < 700, "cos wrapper too large: {cos_words}");
    assert!(
        core_words < 4500,
        "shared core grew too large: {core_words}"
    );
    assert!(
        sin_words + cos_words + core_words < 5540,
        "shared sin+cos helpers should beat duplicated Phase 43 bodies"
    );
}

#[test]
/// Verifies Phase 47 sign-normalized alias matching recovers shared trig core size.
fn phase47_sincos_core_size_recovered_after_alias_compaction() {
    let source = r#"
#include <math.h>

float angle;
float s;
float c;

void main(void) {
    angle = 25.132742f;
    s = sinf(angle);
    c = cosf(angle);
}
"#;
    let (_hex, _stdout, report) = compile_profile_source_size_report(
        "balanced",
        "phase47-sincos-size-recovery",
        source,
        &["--math-profile", "balanced"],
    );
    let core_words = parse_runtime_helper_actual_words(&report, "__rt_f32_sincos_core");
    assert!(
        core_words < 4429,
        "shared core should be below Phase 46 baseline: {core_words}"
    );
    assert!(
        core_words <= 3500,
        "shared core missed preferred Phase 47 target: {core_words}"
    );
}

#[test]
/// Verifies Phase 43 compact/balanced profile reporting and precise deferral for dynamic trig.
fn phase43_sincos_profile_policy_is_explicit() {
    let source = r#"
#include <math.h>

float angle;
float value;

void main(void) {
    angle = 0.7853982f;
    value = sinf(angle);
}
"#;
    let (_compact_hex, _compact_stdout, compact_report) = compile_profile_source_size_report(
        "balanced",
        "phase43-sincos-compact",
        source,
        &["--math-profile", "compact"],
    );
    let (_balanced_hex, _balanced_stdout, balanced_report) = compile_profile_source_size_report(
        "balanced",
        "phase43-sincos-balanced",
        source,
        &["--math-profile", "balanced"],
    );
    assert!(compact_report.contains("variant=shared_core_compact"));
    assert!(compact_report.contains("__rt_math_sin_qwave_table_compact"));
    assert!(balanced_report.contains("variant=shared_core_balanced"));
    assert!(balanced_report.contains("__rt_math_sin_qwave_table_balanced"));

    let precise_error = compile_error_with_extra_args(
        "pic16f877a",
        "phase43-sincos-precise-deferred.c",
        r#"
#include <math.h>

float angle;
float value;

void main(void) {
    angle = 1.5707963f;
    value = sinf(angle);
}
"#,
        &["--math-profile", "precise"],
    );
    assert!(precise_error.contains("sinf") || precise_error.contains("__rt_f32_sin"));
}

#[test]
/// Verifies Phase 43 diagnostics reject wrong sin/cos forms and ISR helper calls.
fn phase43_sincos_diagnostics_are_explicit() {
    let wrong_count = compile_error(
        "pic16f877a",
        "phase43-sinf-wrong-count.c",
        r#"
#include <math.h>

float value;

void main(void) {
    value = sinf();
}
"#,
    );
    assert!(wrong_count.contains("expects 1 argument"));

    let wrong_type = compile_error(
        "pic16f877a",
        "phase43-cosf-wrong-type.c",
        r#"
#include <math.h>

float value;

void main(void) {
    int input = 1;
    value = cosf(input);
}
"#,
    );
    assert!(wrong_type.contains("expects a float argument"));

    let unsupported_double = compile_error(
        "pic16f877a",
        "phase43-sin-unsupported.c",
        r#"
#include <math.h>

float value;

void main(void) {
    value = sin(1.0f);
}
"#,
    );
    assert!(unsupported_double.contains("unsupported double math function `sin`"));

    let isr = compile_error(
        "pic16f877a",
        "phase43-sinf-isr.c",
        r#"
#include <math.h>

float value;

void __interrupt isr(void) {
    value = sinf(value);
}

void main(void) {}
"#,
    );
    assert!(isr.contains("cannot call `sinf`"));
    assert!(isr.contains("float math helper"));
}

#[test]
/// Verifies checked-in Phase 43 sin/cos examples compile and report resources.
fn phase43_sincos_examples_compile_via_picc() {
    for example in [
        "examples/pic16f877a/math_sincos_basic.c",
        "examples/pic16f877a/math_sincos_profile_compare.c",
        "examples/pic16f877a/math_sincos_rom_input.c",
        "examples/pic16f877a/math_sincos_resource_report.c",
        "examples/pic16f877a/math_sincos_shared_core.c",
        "examples/pic16f877a/math_sincos_size_compare.c",
        "examples/pic16f877a/math_sincos_runtime_profiles.c",
        "examples/pic16f877a/math_sincos_accuracy.c",
        "examples/pic16f877a/math_sincos_range_reduction.c",
        "examples/pic16f877a/math_sincos_compact_vs_balanced.c",
        "examples/pic16f877a/math_sincos_range_moderate.c",
        "examples/pic16f877a/math_sincos_range_profiles.c",
        "examples/pic16f877a/math_sincos_range_resource_report.c",
        "examples/pic16f877a/math_sincos_core_size_recovery.c",
        "examples/pic16f877a/math_sincos_alias_compaction.c",
        "examples/pic16f877a/math_sincos_phase47_resource_report.c",
    ] {
        let output = compile_example_via_picc_cli_with_extra_args(
            "pic16f877a",
            example,
            &["--size", "--memory-report", "--verify-hex"],
        );
        assert_hex_is_programmable(&output);
    }
}
