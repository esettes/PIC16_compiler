use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use pic16cc::cli::{CliCommand, CliOptions, CompileCommand, OptimizationLevel, OutputArtifacts};
use pic16cc::diagnostics::WarningProfile;
use pic16cc::execute;

fn repo(path: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(path)
}

fn temp_file(name: &str) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time")
        .as_nanos();
    std::env::temp_dir().join(format!("pic16cc-{stamp}-{name}"))
}

fn compile_example(target: &str, input: &str) -> PathBuf {
    let output = temp_file("out.hex");
    let options = CliOptions {
        command: CliCommand::Compile(CompileCommand {
            target: target.to_string(),
            input: repo(input),
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

#[test]
fn compiles_pic16f628a_blink() {
    let output = compile_example("pic16f628a", "examples/pic16f628a/blink.c");
    let hex = fs::read_to_string(&output).expect("hex");
    assert!(hex.contains(":00000001FF"));
    assert!(fs::read_to_string(output.with_extension("map"))
        .expect("map")
        .contains("Code Symbols"));
    assert!(fs::read_to_string(output.with_extension("lst"))
        .expect("lst")
        .contains("Assembly"));
}

#[test]
fn compiles_pic16f877a_blink() {
    let output = compile_example("pic16f877a", "examples/pic16f877a/blink.c");
    let asm = fs::read_to_string(output.with_extension("asm")).expect("asm");
    assert!(asm.contains("fn_main"));
    assert!(asm.contains("movwf"));
}

#[test]
fn reports_unsupported_multiply() {
    let source = temp_file("unsupported.c");
    fs::write(
        &source,
        "\
#include <pic16/pic16f628a.h>
void main(void) {
    TRISB = 0x00;
    PORTB = 2 * 3;
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
    assert!(format!("{error}").contains("not yet supported"));
}

#[test]
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
