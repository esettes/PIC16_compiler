// SPDX-License-Identifier: GPL-3.0-or-later

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
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
    std::env::temp_dir().join(format!("pic16cc-sim-cli-{stamp}-{name}"))
}

fn sim_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_pic16-sim"))
}

fn run_sim(args: &[String]) -> Output {
    Command::new(sim_bin())
        .args(args)
        .output()
        .expect("run pic16-sim")
}

fn compile_source(name: &str, source: &str) -> PathBuf {
    let input = temp_file(name);
    fs::write(&input, source).expect("fixture");
    let output = temp_file("out.hex");
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

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

fn sim_args(hex: &Path, extra: &[&str]) -> Vec<String> {
    let mut args = vec![hex.display().to_string()];
    args.extend(extra.iter().map(|arg| (*arg).to_string()));
    args
}

#[test]
fn help_and_version_work() {
    let help = run_sim(&["--help".to_string()]);
    assert!(help.status.success());
    assert!(stdout(&help).contains("Usage:"));

    let version = run_sim(&["--version".to_string()]);
    assert!(version.status.success());
    assert!(stdout(&version).contains("pic16-sim"));
}

#[test]
fn malformed_hex_returns_diagnostic() {
    let hex = temp_file("bad.hex");
    fs::write(&hex, "bad\n").expect("bad hex");
    let output = run_sim(&sim_args(&hex, &[]));

    assert!(!output.status.success());
    assert!(stderr(&output).contains("invalid Intel HEX record"));
}

#[test]
fn missing_map_returns_diagnostic() {
    let hex = compile_source(
        "missing-map.c",
        r#"
unsigned char result;
void main(void) { result = 1; }
"#,
    );
    let output = run_sim(&sim_args(
        &hex,
        &["--map", "/tmp/pic16cc-missing.map", "--run-until", "__halt"],
    ));

    assert!(!output.status.success());
    assert!(stderr(&output).contains("failed to read map"));
}

#[test]
fn unknown_symbol_returns_diagnostic() {
    let hex = compile_source(
        "unknown-symbol.c",
        r#"
unsigned char result;
void main(void) { result = 1; }
"#,
    );
    let map = hex.with_extension("map");
    let output = run_sim(&sim_args(
        &hex,
        &[
            "--map",
            &map.display().to_string(),
            "--run-until",
            "missing_symbol",
        ],
    ));

    assert!(!output.status.success());
    assert!(stderr(&output).contains("unknown symbol `missing_symbol`"));
}

#[test]
fn runs_until_halt_and_prints_arithmetic_symbol() {
    let hex = compile_source(
        "arith.c",
        r#"
unsigned char result;
void main(void) { result = 2 + 3; }
"#,
    );
    let map = hex.with_extension("map");
    let output = run_sim(&sim_args(
        &hex,
        &[
            "--map",
            &map.display().to_string(),
            "--run-until",
            "__halt",
            "--print-symbol",
            "result",
        ],
    ));

    assert!(output.status.success(), "{}", stderr(&output));
    assert_eq!(stdout(&output), "result = 5 (0x05)\n");
}

#[test]
fn max_steps_stop_without_symbol_succeeds() {
    let hex = compile_source(
        "max-steps.c",
        r#"
unsigned char result;
void main(void) { result = 9; }
"#,
    );
    let output = run_sim(&sim_args(&hex, &["--max-steps", "2"]));

    assert!(output.status.success(), "{}", stderr(&output));
    assert!(stdout(&output).contains("stopped after max steps"));
}

#[test]
fn prints_registers_and_trace_stdout() {
    let hex = compile_source(
        "regs-trace.c",
        r#"
unsigned char result;
void main(void) { result = 7; }
"#,
    );
    let map = hex.with_extension("map");
    let output = run_sim(&sim_args(
        &hex,
        &[
            "--map",
            &map.display().to_string(),
            "--run-until",
            "__halt",
            "--trace",
            "--print-regs",
        ],
    ));
    let stdout = stdout(&output);

    assert!(output.status.success(), "{}", stderr(&output));
    assert!(stdout.contains("PC="));
    assert!(stdout.contains("WORD="));
    assert!(stdout.contains("STATUS="));
}

#[test]
fn writes_trace_file() {
    let hex = compile_source(
        "trace-file.c",
        r#"
unsigned char result;
void main(void) { result = 11; }
"#,
    );
    let map = hex.with_extension("map");
    let trace = temp_file("trace.txt");
    let output = run_sim(&sim_args(
        &hex,
        &[
            "--map",
            &map.display().to_string(),
            "--run-until",
            "__halt",
            "--trace-file",
            &trace.display().to_string(),
        ],
    ));

    assert!(output.status.success(), "{}", stderr(&output));
    let trace_text = fs::read_to_string(trace).expect("trace file");
    assert!(trace_text.contains("PC="));
    assert!(trace_text.contains("WORD="));
}

#[test]
fn runs_function_pointer_result() {
    let hex = compile_source(
        "function-pointer.c",
        r#"
unsigned char result;

unsigned char twice(unsigned char value) {
    return value * 2;
}

void main(void) {
    unsigned char (*fn)(unsigned char);
    fn = twice;
    result = fn(21);
}
"#,
    );
    let map = hex.with_extension("map");
    let output = run_sim(&sim_args(
        &hex,
        &[
            "--map",
            &map.display().to_string(),
            "--run-until",
            "__halt",
            "--print-symbol",
            "result",
        ],
    ));

    assert!(output.status.success(), "{}", stderr(&output));
    assert_eq!(stdout(&output), "result = 42 (0x2A)\n");
}

#[test]
fn runs_rom_read_result() {
    let hex = compile_source(
        "rom-read.c",
        r#"
const __rom unsigned char table[] = { 4, 8, 15, 16 };
unsigned char result;

void main(void) {
    result = __rom_read8(table, 2);
}
"#,
    );
    let map = hex.with_extension("map");
    let output = run_sim(&sim_args(
        &hex,
        &[
            "--map",
            &map.display().to_string(),
            "--run-until",
            "__halt",
            "--print-symbol",
            "result",
        ],
    ));

    assert!(output.status.success(), "{}", stderr(&output));
    assert_eq!(stdout(&output), "result = 15 (0x0F)\n");
}
