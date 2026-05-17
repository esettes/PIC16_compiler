// SPDX-License-Identifier: GPL-3.0-or-later

#![forbid(unsafe_code)]

pub mod assembler;
pub mod backend;
pub mod cli;
pub mod common;
pub mod diagnostics;
pub mod frontend;
pub mod hex;
pub mod ir;
pub mod linker;
pub mod sim;
pub mod sim_cli;

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use assembler::listing::render_listing;
use backend::pic16::devices::{DeviceRegistry, TargetDevice};
use backend::pic16::midrange14::codegen::{BackendOptions, StackReportSummary, compile_program};
use cli::{CLI_NAME, CliCommand, CliOptions, OptimizationLevel, ProgramCommand};
use common::source::SourceManager;
use diagnostics::{DiagnosticBag, DiagnosticEmitter, Severity, StageResult};
use frontend::ast::TranslationUnit;
use frontend::lexer::Lexer;
use frontend::parser::Parser;
use frontend::preprocessor::Preprocessor;
use frontend::semantic::SemanticAnalyzer;
use hex::intel_hex::{IntelHexWriter, validate_hex_output};
use ir::lowering::IrLowerer;
use ir::passes::{compact_temps, constant_fold, dead_code_elimination};
use linker::map::render_map;

#[derive(Debug)]
pub struct CompilationOutput {
    pub hex_path: PathBuf,
    pub generated_files: Vec<PathBuf>,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct OptimizationReport {
    constant_fold: ir::passes::ConstantFoldStats,
    dead_code: ir::passes::DeadCodeStats,
    temp_compaction: ir::passes::TempCompactionStats,
    backend: backend::pic16::midrange14::codegen::BackendOptimizationReport,
}

/// Executes the selected CLI command and returns generated output paths on success.
pub fn execute(options: CliOptions) -> StageResult<CompilationOutput> {
    match options.command.clone() {
        CliCommand::Compile(command) => compile_command(command),
        CliCommand::PrintProgramCommand(command) => print_program_command(command),
        CliCommand::ListTargets => {
            let registry = DeviceRegistry::new();
            let mut lines = Vec::new();
            for device in registry.devices() {
                lines.push(format!(
                    "{name:12} program={program}w ram={ram}b modeled_gpr={modeled}b eeprom={eeprom}b banks={banks}",
                    name = device.name,
                    program = device.program_words,
                    ram = device.data_ram_bytes,
                    modeled = device.modeled_data_ram_bytes(),
                    eeprom = device.eeprom_bytes,
                    banks = device.bank_count
                ));
            }
            println!("{}", lines.join("\n"));
            Ok(CompilationOutput {
                hex_path: PathBuf::new(),
                generated_files: Vec::new(),
            })
        }
        CliCommand::Help => {
            print!("{}", cli::help_text());
            Ok(CompilationOutput {
                hex_path: PathBuf::new(),
                generated_files: Vec::new(),
            })
        }
        CliCommand::Version => {
            println!("{CLI_NAME} {}", env!("CARGO_PKG_VERSION"));
            Ok(CompilationOutput {
                hex_path: PathBuf::new(),
                generated_files: Vec::new(),
            })
        }
    }
}

/// Runs the full single-file compilation pipeline from source to Intel HEX.
fn compile_command(command: cli::CompileCommand) -> StageResult<CompilationOutput> {
    let registry = DeviceRegistry::new();
    let target = registry.device(&command.target).ok_or_else(|| {
        DiagnosticBag::single(
            Severity::Error,
            "cli",
            format!("unknown target `{}`", command.target),
        )
    })?;

    let mut diagnostics = DiagnosticBag::new(command.warning_profile);
    let source_path = fs::canonicalize(&command.input).map_err(|error| {
        DiagnosticBag::single(
            Severity::Error,
            "cli",
            format!(
                "failed to resolve input `{}`: {error}",
                command.input.display()
            ),
        )
    })?;

    let mut source_manager = SourceManager::new();
    let main_source = source_manager.load(&source_path).map_err(|error| {
        DiagnosticBag::single(
            Severity::Error,
            "io",
            format!("failed to read `{}`: {error}", source_path.display()),
        )
    })?;

    let mut preprocessor = Preprocessor::new(
        target,
        command.include_dirs.clone(),
        command.defines.clone(),
        &mut source_manager,
    );
    let preprocessed = preprocessor.process(main_source, &mut diagnostics);
    if diagnostics.has_errors() {
        return Err(diagnostics);
    }

    let preprocessed = preprocessed.expect("preprocessor result checked");
    let config_word = target
        .resolve_config_word(&preprocessed.config_directives, &mut diagnostics)
        .unwrap_or(target.default_config_word);
    if diagnostics.has_errors() {
        return Err(diagnostics);
    }
    if command.artifacts.emit_tokens {
        let tokens = Lexer::new(&preprocessed, &mut diagnostics).collect_debug();
        write_artifact(&command.output, "tokens", &tokens)?;
    }

    let mut lexer = Lexer::new(&preprocessed, &mut diagnostics);
    let tokens = lexer.tokenize();
    if diagnostics.has_errors() {
        return Err(diagnostics);
    }

    let mut parser = Parser::new(tokens, &preprocessed, &mut diagnostics);
    let ast: TranslationUnit = parser.parse_translation_unit();
    if diagnostics.has_errors() {
        return Err(diagnostics);
    }

    if command.artifacts.emit_ast {
        write_artifact(&command.output, "ast", &ast.render())?;
    }

    let semantic = SemanticAnalyzer::new(target, command.stack_check);
    let typed_program = semantic.analyze(ast, &mut diagnostics);
    if diagnostics.has_errors() {
        return Err(diagnostics);
    }
    let typed_program = typed_program.expect("semantic result checked");

    let mut ir_program = IrLowerer::new(target).lower(&typed_program, &mut diagnostics);
    if diagnostics.has_errors() {
        return Err(diagnostics);
    }

    let mut optimization_report = OptimizationReport::default();
    match command.optimization {
        OptimizationLevel::O0 => {}
        OptimizationLevel::O1 | OptimizationLevel::O2 | OptimizationLevel::Os => {
            optimization_report.constant_fold = constant_fold(&mut ir_program);
            optimization_report.dead_code = dead_code_elimination(&mut ir_program);
            optimization_report.temp_compaction = compact_temps(&mut ir_program);
        }
    }

    if command.artifacts.emit_ir {
        write_artifact(&command.output, "ir", &ir_program.render())?;
    }

    let assembled = compile_program(
        target,
        &typed_program,
        &ir_program,
        &BackendOptions {
            stack_check: command.stack_check,
            enforce_resource_limits: command.artifacts.size
                || command.artifacts.memory_report
                || command.artifacts.memory_report_file.is_some(),
        },
        &mut diagnostics,
    );
    if diagnostics.has_errors() {
        return Err(diagnostics);
    }
    let mut assembled = assembled.expect("backend result checked");
    optimization_report.backend = assembled.optimization;
    assembled.map.resource_lines.push(format!(
        "Config word: 0x{config_word:04X} @ 0x{:04X}",
        target.vectors.config_word
    ));

    if command.artifacts.emit_asm {
        write_artifact(&command.output, "asm", &assembled.program.render())?;
    }

    let listing_path = command
        .artifacts
        .list_file
        .then(|| change_extension(&command.output, "lst"));
    let map_path = command
        .artifacts
        .map
        .then(|| change_extension(&command.output, "map"));

    if let Some(parent) = command.output.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            DiagnosticBag::single(
                Severity::Error,
                "io",
                format!(
                    "failed to create output directory `{}`: {error}",
                    parent.display()
                ),
            )
        })?;
    }

    if let Some(path) = &listing_path {
        fs::write(
            path,
            render_listing(
                &assembled.program,
                &assembled.words,
                Some(&listing_resource_summary(
                    &assembled.resource_report.size_text,
                    target,
                    config_word,
                )),
            ),
        )
        .map_err(|error| {
            DiagnosticBag::single(
                Severity::Error,
                "io",
                format!("failed to write listing `{}`: {error}", path.display()),
            )
        })?;
    }

    if let Some(path) = &map_path {
        fs::write(path, render_map(&assembled.map)).map_err(|error| {
            DiagnosticBag::single(
                Severity::Error,
                "io",
                format!("failed to write map `{}`: {error}", path.display()),
            )
        })?;
    }

    if let Some(path) = &command.stack_report_file {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|error| {
                DiagnosticBag::single(
                    Severity::Error,
                    "io",
                    format!(
                        "failed to create stack report directory `{}`: {error}",
                        parent.display()
                    ),
                )
            })?;
        }
        fs::write(path, &assembled.stack_report.text).map_err(|error| {
            DiagnosticBag::single(
                Severity::Error,
                "io",
                format!("failed to write stack report `{}`: {error}", path.display()),
            )
        })?;
    }

    if let Some(path) = &command.artifacts.memory_report_file {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|error| {
                DiagnosticBag::single(
                    Severity::Error,
                    "io",
                    format!(
                        "failed to create memory report directory `{}`: {error}",
                        parent.display()
                    ),
                )
            })?;
        }
        fs::write(path, &assembled.resource_report.text).map_err(|error| {
            DiagnosticBag::single(
                Severity::Error,
                "io",
                format!(
                    "failed to write memory report `{}`: {error}",
                    path.display()
                ),
            )
        })?;
    }

    let hex_records = IntelHexWriter::new(target).emit(&assembled.words, config_word);
    let hex_report = match validate_hex_output(target, &assembled.words, config_word, &hex_records)
    {
        Ok(report) => report,
        Err(errors) => {
            let mut bag = DiagnosticBag::new(command.warning_profile);
            for error in errors {
                bag.error("hex", None, error, None);
            }
            return Err(bag);
        }
    };
    fs::write(&command.output, hex_records).map_err(|error| {
        DiagnosticBag::single(
            Severity::Error,
            "io",
            format!(
                "failed to write hex `{}`: {error}",
                command.output.display()
            ),
        )
    })?;

    let emitter = DiagnosticEmitter::new(&source_manager, &preprocessed);
    emitter.print(&diagnostics);
    if diagnostics.has_errors() {
        return Err(diagnostics);
    }

    if command.opt_report {
        print_optimization_report(command.optimization, &optimization_report);
        print_stack_report_summary(&assembled.stack_report.summary);
    }

    if command.stack_report {
        println!("{}", assembled.stack_report.text);
    }

    if command.artifacts.size {
        print!("{}", assembled.resource_report.size_text);
    }

    if command.artifacts.memory_report {
        println!("{}", assembled.resource_report.text);
    }

    if command.artifacts.verify_hex {
        println!("{}", hex_report.text);
    }

    if command.artifacts.program {
        run_programmer_command(&command.output, command.artifacts.program_cmd.as_deref())?;
    }

    let mut generated_files = vec![command.output.clone()];
    if let Some(path) = map_path {
        generated_files.push(path);
    }
    if let Some(path) = listing_path {
        generated_files.push(path);
    }
    if let Some(path) = command.stack_report_file.clone() {
        generated_files.push(path);
    }
    if let Some(path) = command.artifacts.memory_report_file.clone() {
        generated_files.push(path);
    }

    Ok(CompilationOutput {
        hex_path: command.output,
        generated_files,
    })
}

/// Prints one external programmer command without compiling or flashing.
fn print_program_command(command: ProgramCommand) -> StageResult<CompilationOutput> {
    let registry = DeviceRegistry::new();
    let _target = registry.device(&command.target).ok_or_else(|| {
        DiagnosticBag::single(
            Severity::Error,
            "cli",
            format!("unknown target `{}`", command.target),
        )
    })?;
    println!(
        "{}",
        render_program_command(&command.output, command.program_cmd.as_deref())
    );
    Ok(CompilationOutput {
        hex_path: PathBuf::new(),
        generated_files: Vec::new(),
    })
}

fn listing_resource_summary(size_text: &str, target: &TargetDevice, config_word: u16) -> String {
    let mut summary = size_text.to_string();
    summary.push_str(&format!(
        "Config word: 0x{config_word:04X} @ 0x{:04X}\n",
        target.vectors.config_word
    ));
    summary
}

fn render_program_command(output: &Path, program_cmd: Option<&str>) -> String {
    let command = program_cmd.unwrap_or("${FLASH_CMD:-echo \"Configure FLASH_CMD to program\"}");
    format!("{command} ${{FLASH_ARGS:-}} {}", output.display())
}

fn run_programmer_command(output: &Path, program_cmd: Option<&str>) -> StageResult<()> {
    let Some(program_cmd) = program_cmd else {
        return Err(DiagnosticBag::single(
            Severity::Error,
            "cli",
            "programmer command requested but missing; pass `--program-cmd <cmd>`".to_string(),
        ));
    };
    let command_line = format!("{} {}", program_cmd, output.display());
    let status = Command::new("sh")
        .arg("-c")
        .arg(&command_line)
        .status()
        .map_err(|error| {
            DiagnosticBag::single(
                Severity::Error,
                "cli",
                format!("failed to run programmer command `{command_line}`: {error}"),
            )
        })?;
    if status.success() {
        Ok(())
    } else {
        Err(DiagnosticBag::single(
            Severity::Error,
            "cli",
            format!("programmer command failed with status {status}"),
        ))
    }
}

/// Writes an auxiliary compiler artifact next to the main output path.
fn write_artifact(output: &Path, extension: &str, contents: &str) -> StageResult<()> {
    let path = change_extension(output, extension);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            DiagnosticBag::single(
                Severity::Error,
                "io",
                format!(
                    "failed to create artifact directory `{}`: {error}",
                    parent.display()
                ),
            )
        })?;
    }
    fs::write(&path, contents).map_err(|error| {
        DiagnosticBag::single(
            Severity::Error,
            "io",
            format!("failed to write artifact `{}`: {error}", path.display()),
        )
    })
}

/// Replaces the output path extension while preserving the parent directory.
fn change_extension(path: &Path, extension: &str) -> PathBuf {
    path.with_extension(extension)
}

/// Prints a compact optimization summary suitable for `--opt-report`.
fn print_optimization_report(level: OptimizationLevel, report: &OptimizationReport) {
    println!("Optimization report ({level})");
    println!(
        "  IR constant propagation/folding: propagated={} folded={} simplified_branches={} pruned_unreachable={}",
        report.constant_fold.operands_propagated,
        report.constant_fold.expressions_folded,
        report.constant_fold.branches_simplified,
        report.constant_fold.unreachable_blocks_pruned
    );
    println!(
        "  IR dead code elimination: removed_instructions={} cleared_unreachable_blocks={}",
        report.dead_code.instructions_removed, report.dead_code.unreachable_blocks_cleared
    );
    println!(
        "  Temp compaction: removed_temp_slots={}",
        report.temp_compaction.temp_slots_removed
    );
    println!(
        "  Backend peephole: removed_instructions={} self_moves={} duplicate_writes={} duplicate_bit_ops={} duplicate_setpages={} overwritten_w_loads={}",
        report.backend.peephole.removed_instructions,
        report.backend.peephole.self_moves_removed,
        report.backend.peephole.duplicate_writes_removed,
        report.backend.peephole.duplicate_bit_ops_removed,
        report.backend.peephole.duplicate_setpages_removed,
        report.backend.peephole.overwritten_w_loads_removed
    );
    println!(
        "  Helper calls avoided: {}",
        report.backend.helper_calls_avoided
    );
    println!(
        "  Linker relaxation: passes={} removed_setpages={} same_page_transitions={}",
        report.backend.relaxation.passes,
        report.backend.relaxation.removed_redundant_setpages,
        report.backend.relaxation.relaxed_same_page_transitions
    );
}

/// Prints one compact stack summary suitable for `--opt-report`.
fn print_stack_report_summary(summary: &StackReportSummary) {
    println!("Stack summary");
    println!(
        "  bounds: base=0x{:04X} limit=0x{:04X} capacity={} check={}",
        summary.stack_base,
        summary.stack_limit,
        summary.stack_capacity,
        if summary.stack_check { "on" } else { "off" }
    );
    println!(
        "  static usage: max_bytes={} max_call_depth={} isr_frame={} isr_context={}",
        summary.static_max_stack,
        summary.max_call_depth,
        summary.isr_frame_bytes,
        summary.isr_context_bytes
    );
    println!(
        "  function-pointer groups: {} total_targets={} unknown_target_sets={}",
        summary.function_pointer_groups,
        summary.function_pointer_targets,
        summary.unknown_function_pointer_target_sets
    );
}

/// Returns predefined macros that describe the compiler and active target.
pub fn default_predefined_macros(device: &TargetDevice) -> BTreeMap<String, String> {
    let mut macros = BTreeMap::new();
    macros.insert("__pic16cc__".to_string(), "1".to_string());
    macros.insert(
        format!("__{}__", device.name.to_ascii_uppercase()),
        "1".to_string(),
    );
    macros
}
// SPDX-License-Identifier: GPL-3.0-or-later
