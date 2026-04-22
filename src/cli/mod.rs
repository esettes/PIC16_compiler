use std::collections::BTreeMap;
use std::fmt::{Display, Formatter};
use std::path::PathBuf;

use crate::diagnostics::WarningProfile;

#[derive(Clone, Debug)]
pub struct CliOptions {
    pub command: CliCommand,
}

#[derive(Clone, Debug)]
pub enum CliCommand {
    Compile(CompileCommand),
    ListTargets,
    Help,
    Version,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OptimizationLevel {
    O0,
    O1,
    O2,
    Os,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct OutputArtifacts {
    pub emit_tokens: bool,
    pub emit_ast: bool,
    pub emit_ir: bool,
    pub emit_asm: bool,
    pub map: bool,
    pub list_file: bool,
}

#[derive(Clone, Debug)]
pub struct CompileCommand {
    pub target: String,
    pub input: PathBuf,
    pub output: PathBuf,
    pub include_dirs: Vec<PathBuf>,
    pub defines: BTreeMap<String, String>,
    pub optimization: OptimizationLevel,
    pub artifacts: OutputArtifacts,
    pub verbose: bool,
    pub warning_profile: WarningProfile,
}

impl CliOptions {
    pub fn parse(args: Vec<String>) -> Result<Self, String> {
        let mut iter = args.into_iter();
        let _program = iter.next();

        let mut target = None::<String>;
        let mut output = None::<PathBuf>;
        let mut input = None::<PathBuf>;
        let mut include_dirs = Vec::new();
        let mut defines = BTreeMap::new();
        let mut optimization = OptimizationLevel::O0;
        let mut verbose = false;
        let mut artifacts = OutputArtifacts::default();
        let mut warning_profile = WarningProfile::default();

        while let Some(argument) = iter.next() {
            match argument.as_str() {
                "--help" | "-h" => return Ok(Self { command: CliCommand::Help }),
                "--version" => return Ok(Self { command: CliCommand::Version }),
                "--list-targets" => return Ok(Self { command: CliCommand::ListTargets }),
                "--emit-tokens" => artifacts.emit_tokens = true,
                "--emit-ast" => artifacts.emit_ast = true,
                "--emit-ir" => artifacts.emit_ir = true,
                "--emit-asm" => artifacts.emit_asm = true,
                "--map" => artifacts.map = true,
                "--list-file" => artifacts.list_file = true,
                "--verbose" => verbose = true,
                "-Wall" => warning_profile.wall = true,
                "-Wextra" => warning_profile.wextra = true,
                "-Werror" => warning_profile.werror = true,
                "-O0" => optimization = OptimizationLevel::O0,
                "-O1" => optimization = OptimizationLevel::O1,
                "-O2" => optimization = OptimizationLevel::O2,
                "-Os" => optimization = OptimizationLevel::Os,
                "--target" => {
                    let value = iter
                        .next()
                        .ok_or_else(|| "--target requires a value".to_string())?;
                    target = Some(value);
                }
                "-o" => {
                    let value = iter
                        .next()
                        .ok_or_else(|| "-o requires a value".to_string())?;
                    output = Some(PathBuf::from(value));
                }
                "-I" => {
                    let value = iter
                        .next()
                        .ok_or_else(|| "-I requires a value".to_string())?;
                    include_dirs.push(PathBuf::from(value));
                }
                "-D" => {
                    let value = iter
                        .next()
                        .ok_or_else(|| "-D requires a value".to_string())?;
                    parse_define(&value, &mut defines)?;
                }
                _ if argument.starts_with("-I") => include_dirs.push(PathBuf::from(&argument[2..])),
                _ if argument.starts_with("-D") => parse_define(&argument[2..], &mut defines)?,
                _ if argument.starts_with('-') => {
                    return Err(format!("unknown option `{argument}`\n\n{}", help_text()));
                }
                _ => {
                    if input.is_none() {
                        input = Some(PathBuf::from(argument));
                    } else {
                        return Err("only one input source file is supported in v0.1".to_string());
                    }
                }
            }
        }

        if input.is_none() {
            return Err(format!("missing input file\n\n{}", help_text()));
        }
        if target.is_none() {
            return Err(format!("missing --target\n\n{}", help_text()));
        }

        let input = input.expect("checked");
        let output = output.unwrap_or_else(|| PathBuf::from("a.hex"));

        Ok(Self {
            command: CliCommand::Compile(CompileCommand {
                target: target.expect("checked"),
                input,
                output,
                include_dirs,
                defines,
                optimization,
                artifacts,
                verbose,
                warning_profile,
            }),
        })
    }
}

fn parse_define(raw: &str, defines: &mut BTreeMap<String, String>) -> Result<(), String> {
    if raw.is_empty() {
        return Err("empty -D value".to_string());
    }
    let (name, value) = if let Some((name, value)) = raw.split_once('=') {
        (name, value)
    } else {
        (raw, "1")
    };
    if name.contains('(') {
        return Err(format!(
            "function-like macro `{name}` unsupported in v0.1; only object-like #define/-D macros"
        ));
    }
    defines.insert(name.to_string(), value.to_string());
    Ok(())
}

pub fn help_text() -> &'static str {
    "\
pic16cc 0.1.0

Usage:
  pic16cc --target <name> [options] -o <out.hex> <input.c>
  pic16cc --list-targets
  pic16cc --help
  pic16cc --version

Options:
  --target <name>   Target device (`pic16f628a`, `pic16f877a`)
  -o <path>         Output Intel HEX path
  -I <dir>          Add include directory
  -D <name=value>   Define object-like macro
  -O0|-O1|-O2|-Os   Optimization level
  -Wall             Enable baseline warnings
  -Wextra           Enable extra warnings
  -Werror           Treat warnings as errors
  --emit-tokens     Write token dump next to output
  --emit-ast        Write AST dump next to output
  --emit-ir         Write IR dump next to output
  --emit-asm        Write assembly dump next to output
  --map             Write map file next to output
  --list-file       Write listing file next to output
  --verbose         Enable verbose build logs"
}

impl Display for OptimizationLevel {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        let text = match self {
            Self::O0 => "O0",
            Self::O1 => "O1",
            Self::O2 => "O2",
            Self::Os => "Os",
        };
        formatter.write_str(text)
    }
}
