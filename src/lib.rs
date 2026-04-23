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

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use assembler::listing::render_listing;
use backend::pic16::devices::{DeviceRegistry, TargetDevice};
use backend::pic16::midrange14::codegen::compile_program;
use cli::{CliCommand, CliOptions, OptimizationLevel};
use common::source::SourceManager;
use diagnostics::{DiagnosticBag, DiagnosticEmitter, Severity, StageResult};
use frontend::ast::TranslationUnit;
use frontend::lexer::Lexer;
use frontend::parser::Parser;
use frontend::preprocessor::Preprocessor;
use frontend::semantic::SemanticAnalyzer;
use hex::intel_hex::IntelHexWriter;
use ir::lowering::IrLowerer;
use ir::passes::{constant_fold, dead_code_elimination};
use linker::map::render_map;

#[derive(Debug)]
pub struct CompilationOutput {
    pub hex_path: PathBuf,
    pub generated_files: Vec<PathBuf>,
}

/// Executes the selected CLI command and returns generated output paths on success.
pub fn execute(options: CliOptions) -> StageResult<CompilationOutput> {
    match options.command.clone() {
        CliCommand::Compile(command) => compile_command(command),
        CliCommand::ListTargets => {
            let registry = DeviceRegistry::new();
            let mut lines = Vec::new();
            for device in registry.devices() {
                lines.push(format!(
                    "{name:12} program={program}w ram={ram}b eeprom={eeprom}b banks={banks}",
                    name = device.name,
                    program = device.program_words,
                    ram = device.data_ram_bytes,
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
            println!("pic16cc {}", env!("CARGO_PKG_VERSION"));
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
            format!("failed to resolve input `{}`: {error}", command.input.display()),
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

    let semantic = SemanticAnalyzer::new(target);
    let typed_program = semantic.analyze(ast, &mut diagnostics);
    if diagnostics.has_errors() {
        return Err(diagnostics);
    }
    let typed_program = typed_program.expect("semantic result checked");

    let mut ir_program = IrLowerer::new(target).lower(&typed_program, &mut diagnostics);
    if diagnostics.has_errors() {
        return Err(diagnostics);
    }

    match command.optimization {
        OptimizationLevel::O0 => {}
        OptimizationLevel::O1 | OptimizationLevel::O2 | OptimizationLevel::Os => {
            constant_fold(&mut ir_program);
            dead_code_elimination(&mut ir_program);
        }
    }

    if command.artifacts.emit_ir {
        write_artifact(&command.output, "ir", &ir_program.render())?;
    }

    let assembled = compile_program(target, &typed_program, &ir_program, &mut diagnostics);
    if diagnostics.has_errors() {
        return Err(diagnostics);
    }
    let assembled = assembled.expect("backend result checked");

    if command.artifacts.emit_asm {
        write_artifact(&command.output, "asm", &assembled.program.render())?;
    }

    let listing_path = command.artifacts.list_file.then(|| change_extension(&command.output, "lst"));
    let map_path = command.artifacts.map.then(|| change_extension(&command.output, "map"));

    if let Some(path) = &listing_path {
        fs::write(path, render_listing(&assembled.program, &assembled.words)).map_err(|error| {
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

    let hex_records = IntelHexWriter::new(target).emit(&assembled.words, target.default_config_word);
    if let Some(parent) = command.output.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            DiagnosticBag::single(
                Severity::Error,
                "io",
                format!("failed to create output directory `{}`: {error}", parent.display()),
            )
        })?;
    }
    fs::write(&command.output, hex_records).map_err(|error| {
        DiagnosticBag::single(
            Severity::Error,
            "io",
            format!("failed to write hex `{}`: {error}", command.output.display()),
        )
    })?;

    let emitter = DiagnosticEmitter::new(&source_manager, &preprocessed);
    emitter.print(&diagnostics);
    if diagnostics.has_errors() {
        return Err(diagnostics);
    }

    let mut generated_files = vec![command.output.clone()];
    if let Some(path) = map_path {
        generated_files.push(path);
    }
    if let Some(path) = listing_path {
        generated_files.push(path);
    }

    Ok(CompilationOutput {
        hex_path: command.output,
        generated_files,
    })
}

/// Writes an auxiliary compiler artifact next to the main output path.
fn write_artifact(output: &Path, extension: &str, contents: &str) -> StageResult<()> {
    let path = change_extension(output, extension);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            DiagnosticBag::single(
                Severity::Error,
                "io",
                format!("failed to create artifact directory `{}`: {error}", parent.display()),
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
