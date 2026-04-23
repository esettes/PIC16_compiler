use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
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
/// Verifies unsupported multiply operations fail with a clear Phase 2 diagnostic.
fn reports_unsupported_multiply() {
    let source = temp_file("unsupported.c");
    fs::write(
        &source,
        "\
#include <pic16/pic16f628a.h>
unsigned int scale(unsigned int lhs, unsigned int rhs) {
    return lhs * rhs;
}
void main(void) {
    TRISB = 0x00;
    PORTB = scale(2, 3);
}
",
    )
    .expect("fixture");

    let options = CliOptions {
        command: CliCommand::Compile(CompileCommand {
            target: "pic16f628a".to_string(),
            input: source,
            output: temp_file("unsupported.hex"),
            include_dirs: vec![repo("include")],
            defines: BTreeMap::new(),
            optimization: OptimizationLevel::O0,
            artifacts: OutputArtifacts::default(),
            verbose: false,
            warning_profile: WarningProfile::default(),
        }),
    };

    let error = execute(options).expect_err("must fail");
    assert!(format!("{error}").contains("not implemented in phase 2"));
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
            warning_profile: WarningProfile::default(),
        }),
    })
    .expect_err("must fail");

    assert!(format!("{error}").contains("mixed signedness"));
}

#[test]
/// Verifies pointer-to-pointer declarations are rejected instead of lowering partially.
fn reports_unsupported_pointer_to_pointer() {
    let error = execute(CliOptions {
        command: CliCommand::Compile(CompileCommand {
            target: "pic16f628a".to_string(),
            input: {
                let input = temp_file("ptrptr.c");
                fs::write(
                    &input,
                    "\
#include <pic16/pic16f628a.h>
void main(void) {
    unsigned char **pp;
    TRISB = 0x00;
    PORTB = 0x00;
}
",
                )
                .expect("fixture");
                input
            },
            output: temp_file("ptrptr.hex"),
            include_dirs: vec![repo("include")],
            defines: BTreeMap::new(),
            optimization: OptimizationLevel::O0,
            artifacts: OutputArtifacts::default(),
            verbose: false,
            warning_profile: WarningProfile::default(),
        }),
    })
    .expect_err("must fail");

    assert!(format!("{error}").contains("unsupported type"));
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
            warning_profile: WarningProfile::default(),
        }),
    })
    .expect_err("must fail");

    assert!(format!("{error}").contains("multidimensional arrays"));
}

#[test]
/// Verifies unsupported pointer arithmetic forms are rejected clearly.
fn reports_unsupported_pointer_pointer_subtraction() {
    let error = execute(CliOptions {
        command: CliCommand::Compile(CompileCommand {
            target: "pic16f628a".to_string(),
            input: {
                let input = temp_file("ptrsub.c");
                fs::write(
                    &input,
                    "\
#include <pic16/pic16f628a.h>
unsigned char bytes[2];
void main(void) {
    unsigned char *lhs = bytes;
    unsigned char *rhs = bytes;
    TRISB = 0x00;
    if ((lhs - rhs) != 0) {
        PORTB = 0x01;
    }
}
",
                )
                .expect("fixture");
                input
            },
            output: temp_file("ptrsub.hex"),
            include_dirs: vec![repo("include")],
            defines: BTreeMap::new(),
            optimization: OptimizationLevel::O0,
            artifacts: OutputArtifacts::default(),
            verbose: false,
            warning_profile: WarningProfile::default(),
        }),
    })
    .expect_err("must fail");

    assert!(format!("{error}").contains("unsupported pointer arithmetic form"));
}

#[test]
/// Verifies array initializers fail explicitly instead of being silently ignored.
fn reports_unsupported_array_initializer() {
    let error = execute(CliOptions {
        command: CliCommand::Compile(CompileCommand {
            target: "pic16f628a".to_string(),
            input: {
                let input = temp_file("array-init.c");
                fs::write(
                    &input,
                    "\
#include <pic16/pic16f628a.h>
unsigned char bytes[2] = 0;
void main(void) {
    TRISB = 0x00;
}
",
                )
                .expect("fixture");
                input
            },
            output: temp_file("array-init.hex"),
            include_dirs: vec![repo("include")],
            defines: BTreeMap::new(),
            optimization: OptimizationLevel::O0,
            artifacts: OutputArtifacts::default(),
            verbose: false,
            warning_profile: WarningProfile::default(),
        }),
    })
    .expect_err("must fail");

    assert!(format!("{error}").contains("array initializers"));
}

#[test]
/// Verifies the README command shape still parses into the expected CLI options.
fn parses_cli_shape_requested_in_readme() {
    let options = CliOptions::parse(vec![
        "pic16cc".to_string(),
        "--target".to_string(),
        "pic16f628a".to_string(),
        "-I".to_string(),
        "include".to_string(),
        "-O2".to_string(),
        "-Wall".to_string(),
        "-Wextra".to_string(),
        "--emit-ast".to_string(),
        "--emit-ir".to_string(),
        "--emit-asm".to_string(),
        "--map".to_string(),
        "--list-file".to_string(),
        "-o".to_string(),
        "build/blink.hex".to_string(),
        "examples/pic16f628a/blink.c".to_string(),
    ])
    .expect("parse cli");

    let CliCommand::Compile(command) = options.command else {
        panic!("expected compile command");
    };
    assert_eq!(command.target, "pic16f628a");
    assert_eq!(command.output, PathBuf::from("build/blink.hex"));
    assert!(command.artifacts.emit_ast);
    assert!(command.artifacts.emit_ir);
    assert!(command.artifacts.emit_asm);
    assert!(command.artifacts.map);
    assert!(command.artifacts.list_file);
}
