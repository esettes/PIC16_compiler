// SPDX-License-Identifier: GPL-3.0-or-later

use std::collections::BTreeMap;
use std::fmt::{Display, Formatter, Write as FmtWrite};
use std::fs;
use std::path::{Path, PathBuf};

use crate::backend::pic16::devices::DeviceRegistry;
use crate::sim::{Pic16Core, ProgramImage, SimError, TraceRecord};

pub const SIM_CLI_NAME: &str = "pic16-sim";
const DEFAULT_TARGET: &str = "pic16f877a";
const DEFAULT_MAX_STEPS: u64 = 200_000;

#[derive(Clone, Debug)]
pub struct SimCliOptions {
    pub command: SimCliCommand,
}

#[derive(Clone, Debug)]
pub enum SimCliCommand {
    Run(SimRunCommand),
    Help,
    Version,
}

#[derive(Clone, Debug)]
pub struct SimRunCommand {
    pub hex_path: PathBuf,
    pub map_path: Option<PathBuf>,
    pub target: String,
    pub run_until: Option<String>,
    pub max_steps: u64,
    pub print_symbols: Vec<String>,
    pub print_regs: bool,
    pub trace_stdout: bool,
    pub trace_file: Option<PathBuf>,
}

#[derive(Clone, Debug, Default)]
pub struct SimMap {
    pub code_symbols: BTreeMap<String, u16>,
    pub data_symbols: BTreeMap<String, u16>,
    pub rom_symbols: BTreeMap<String, u16>,
}

#[derive(Debug)]
pub enum SimCliError {
    Message(String),
    Io {
        action: &'static str,
        path: PathBuf,
        error: std::io::Error,
    },
    InvalidMapFile {
        path: PathBuf,
        message: String,
    },
    UnknownSymbol(String),
    Sim(SimError),
}

impl SimCliOptions {
    pub fn parse(args: Vec<String>) -> Result<Self, SimCliError> {
        let mut iter = args.into_iter();
        let _program = iter.next();

        let mut hex_path = None::<PathBuf>;
        let mut map_path = None::<PathBuf>;
        let mut target = DEFAULT_TARGET.to_string();
        let mut run_until = None::<String>;
        let mut max_steps = DEFAULT_MAX_STEPS;
        let mut print_symbols = Vec::new();
        let mut print_regs = false;
        let mut trace_stdout = false;
        let mut trace_file = None::<PathBuf>;

        while let Some(argument) = iter.next() {
            match argument.as_str() {
                "--help" | "-h" => {
                    return Ok(Self {
                        command: SimCliCommand::Help,
                    });
                }
                "--version" => {
                    return Ok(Self {
                        command: SimCliCommand::Version,
                    });
                }
                "--map" => map_path = Some(required_path(&mut iter, "--map")?),
                "--target" => target = required_value(&mut iter, "--target")?,
                "--run-until" => run_until = Some(required_value(&mut iter, "--run-until")?),
                "--max-steps" => {
                    let raw = required_value(&mut iter, "--max-steps")?;
                    max_steps = raw.parse::<u64>().map_err(|_| {
                        SimCliError::Message(format!(
                            "--max-steps requires an unsigned integer, got `{raw}`"
                        ))
                    })?;
                }
                "--print-symbol" => {
                    print_symbols.push(required_value(&mut iter, "--print-symbol")?)
                }
                "--print-regs" => print_regs = true,
                "--trace" => trace_stdout = true,
                "--trace-file" => trace_file = Some(required_path(&mut iter, "--trace-file")?),
                _ if argument.starts_with('-') => {
                    return Err(SimCliError::Message(format!(
                        "unknown option `{argument}`\n\n{}",
                        help_text()
                    )));
                }
                _ => {
                    if hex_path.is_some() {
                        return Err(SimCliError::Message(
                            "only one Intel HEX input file is supported".to_string(),
                        ));
                    }
                    hex_path = Some(PathBuf::from(argument));
                }
            }
        }

        let hex_path = hex_path.ok_or_else(|| {
            SimCliError::Message(format!("missing Intel HEX input file\n\n{}", help_text()))
        })?;

        Ok(Self {
            command: SimCliCommand::Run(SimRunCommand {
                hex_path,
                map_path,
                target,
                run_until,
                max_steps,
                print_symbols,
                print_regs,
                trace_stdout,
                trace_file,
            }),
        })
    }
}

pub fn run(options: SimCliOptions) -> Result<String, SimCliError> {
    match options.command {
        SimCliCommand::Help => Ok(help_text().to_string()),
        SimCliCommand::Version => Ok(format!("{SIM_CLI_NAME} {}\n", env!("CARGO_PKG_VERSION"))),
        SimCliCommand::Run(command) => run_command(&command),
    }
}

pub fn help_text() -> &'static str {
    concat!(
        "pic16-sim ",
        env!("CARGO_PKG_VERSION"),
        "\n\nUsage:\n",
        "  pic16-sim [options] <program.hex>\n",
        "  pic16-sim --help\n",
        "  pic16-sim --version\n\n",
        "Options:\n",
        "  --target <name>        Target device (`pic16f628a`, `pic16f877a`; default `pic16f877a`)\n",
        "  --map <file>           Read picc map file for symbols\n",
        "  --run-until <symbol>   Stop before executing a code symbol such as `__halt`\n",
        "  --max-steps <n>        Maximum instructions to execute (default 200000)\n",
        "  --print-symbol <name>  Print one RAM/SFR symbol value; repeatable\n",
        "  --print-regs           Print core register state after execution\n",
        "  --trace                Print instruction trace to stdout\n",
        "  --trace-file <path>    Write instruction trace to a file\n",
        "  --help                 Show this help\n",
        "  --version              Show version"
    )
}

impl SimMap {
    pub fn from_file(path: &Path) -> Result<Self, SimCliError> {
        let text = fs::read_to_string(path).map_err(|error| SimCliError::Io {
            action: "read map",
            path: path.to_path_buf(),
            error,
        })?;
        Self::parse(path, &text)
    }

    pub fn parse(path: &Path, text: &str) -> Result<Self, SimCliError> {
        let mut map = Self::default();
        let mut section = None::<MapSection>;
        let mut saw_section = false;

        for raw in text.lines() {
            let line = raw.trim();
            match line {
                "Code Symbols" => {
                    section = Some(MapSection::Code);
                    saw_section = true;
                    continue;
                }
                "Data Symbols" => {
                    section = Some(MapSection::Data);
                    saw_section = true;
                    continue;
                }
                "ROM Symbols" => {
                    section = Some(MapSection::Rom);
                    saw_section = true;
                    continue;
                }
                _ => {}
            }

            let Some((addr, name)) = parse_symbol_line(line) else {
                continue;
            };
            match section {
                Some(MapSection::Code) => {
                    map.code_symbols.insert(name, addr);
                }
                Some(MapSection::Data) => {
                    map.data_symbols.insert(name, addr);
                }
                Some(MapSection::Rom) => {
                    map.rom_symbols.insert(name, addr);
                }
                None => {
                    return Err(SimCliError::InvalidMapFile {
                        path: path.to_path_buf(),
                        message: "symbol entry appears before a map section".to_string(),
                    });
                }
            }
        }

        if !saw_section {
            return Err(SimCliError::InvalidMapFile {
                path: path.to_path_buf(),
                message: "missing Code Symbols/Data Symbols/ROM Symbols sections".to_string(),
            });
        }

        Ok(map)
    }

    pub fn code_symbol(&self, name: &str) -> Option<u16> {
        self.code_symbols.get(name).copied()
    }

    pub fn data_symbol(&self, name: &str) -> Option<u16> {
        self.data_symbols.get(name).copied()
    }
}

#[derive(Clone, Copy, Debug)]
enum MapSection {
    Code,
    Data,
    Rom,
}

fn run_command(command: &SimRunCommand) -> Result<String, SimCliError> {
    let registry = DeviceRegistry::new();
    let target = registry
        .device(&command.target)
        .ok_or_else(|| SimCliError::Message(format!("unknown target `{}`", command.target)))?;
    let map = load_map_if_needed(command)?;
    let program = ProgramImage::from_hex_file(&command.hex_path).map_err(SimCliError::Sim)?;
    if program.is_empty() {
        return Err(SimCliError::Message(format!(
            "Intel HEX `{}` contains no loadable instruction words",
            command.hex_path.display()
        )));
    }
    let mut core = Pic16Core::new(target, program);
    let mut output = String::new();
    let mut trace = String::new();

    let stop_pc = if let Some(symbol) = &command.run_until {
        Some(
            map.as_ref()
                .and_then(|symbols| symbols.code_symbol(symbol))
                .ok_or_else(|| SimCliError::UnknownSymbol(symbol.clone()))?,
        )
    } else {
        None
    };

    run_core(command, &mut core, stop_pc, &mut output, &mut trace)?;

    if let Some(path) = &command.trace_file {
        if let Some(parent) = path.parent()
            && !parent.as_os_str().is_empty()
        {
            fs::create_dir_all(parent).map_err(|error| SimCliError::Io {
                action: "create trace directory",
                path: parent.to_path_buf(),
                error,
            })?;
        }
        fs::write(path, &trace).map_err(|error| SimCliError::Io {
            action: "write trace",
            path: path.clone(),
            error,
        })?;
    }

    if command.print_regs {
        render_registers(&mut output, &core);
    }

    for symbol in &command.print_symbols {
        render_symbol(&mut output, &core, target, map.as_ref(), symbol)?;
    }

    if output.is_empty() {
        render_stop_summary(&mut output, command, &core);
    }

    Ok(output)
}

fn load_map_if_needed(command: &SimRunCommand) -> Result<Option<SimMap>, SimCliError> {
    if let Some(path) = &command.map_path {
        return SimMap::from_file(path).map(Some);
    }
    if command.run_until.is_some() || !command.print_symbols.is_empty() {
        return Err(SimCliError::Message(
            "--map <file> is required for --run-until and --print-symbol".to_string(),
        ));
    }
    Ok(None)
}

fn run_core(
    command: &SimRunCommand,
    core: &mut Pic16Core,
    stop_pc: Option<u16>,
    output: &mut String,
    trace: &mut String,
) -> Result<(), SimCliError> {
    let trace_enabled = command.trace_stdout || command.trace_file.is_some();
    if let Some(stop_pc) = stop_pc {
        while core.pc() != stop_pc {
            if core.steps() >= command.max_steps {
                return Err(SimCliError::Sim(SimError::StepLimitExceeded {
                    pc: core.pc(),
                    max_steps: command.max_steps,
                }));
            }
            step_core(core, trace_enabled, command.trace_stdout, output, trace)?;
        }
        return Ok(());
    }

    while core.steps() < command.max_steps {
        step_core(core, trace_enabled, command.trace_stdout, output, trace)?;
    }
    Ok(())
}

fn step_core(
    core: &mut Pic16Core,
    trace_enabled: bool,
    trace_stdout: bool,
    output: &mut String,
    trace: &mut String,
) -> Result<(), SimCliError> {
    if trace_enabled {
        let record = core.step_with_trace().map_err(SimCliError::Sim)?;
        render_trace_record(&record, trace_stdout, output, trace);
    } else {
        core.step().map_err(SimCliError::Sim)?;
    }
    Ok(())
}

fn render_trace_record(
    record: &TraceRecord,
    trace_stdout: bool,
    output: &mut String,
    trace: &mut String,
) {
    let _ = writeln!(trace, "{record}");
    if trace_stdout {
        let _ = writeln!(output, "{record}");
    }
}

fn render_registers(output: &mut String, core: &Pic16Core) {
    let state = core.state();
    let stack_top = state
        .hardware_stack_top
        .map_or_else(|| "empty".to_string(), |pc| format!("0x{pc:04X}"));
    let _ = writeln!(
        output,
        "PC=0x{pc:04X} W=0x{w:02X} STATUS=0x{status:02X} PCLATH=0x{pclath:02X} FSR=0x{fsr:02X} STEPS={steps} STACK_TOP={stack_top}",
        pc = state.pc,
        w = state.w,
        status = state.status,
        pclath = state.pclath,
        fsr = state.fsr,
        steps = state.steps
    );
}

fn render_symbol(
    output: &mut String,
    core: &Pic16Core,
    target: &crate::backend::pic16::devices::TargetDevice,
    map: Option<&SimMap>,
    symbol: &str,
) -> Result<(), SimCliError> {
    let addr = map
        .and_then(|symbols| symbols.data_symbol(symbol))
        .or_else(|| target.sfr_address(symbol))
        .ok_or_else(|| SimCliError::UnknownSymbol(symbol.to_string()))?;
    let value = core.read_data(addr);
    let _ = writeln!(output, "{symbol} = {value} (0x{value:02X})");
    Ok(())
}

fn render_stop_summary(output: &mut String, command: &SimRunCommand, core: &Pic16Core) {
    if let Some(symbol) = &command.run_until {
        let _ = writeln!(
            output,
            "stopped at {symbol} PC=0x{pc:04X} steps={steps}",
            pc = core.pc(),
            steps = core.steps()
        );
    } else {
        let _ = writeln!(
            output,
            "stopped after max steps PC=0x{pc:04X} steps={steps}",
            pc = core.pc(),
            steps = core.steps()
        );
    }
}

fn parse_symbol_line(line: &str) -> Option<(u16, String)> {
    let mut parts = line.split_whitespace();
    let addr = parts.next()?;
    if addr.len() != 4 || !addr.chars().all(|ch| ch.is_ascii_hexdigit()) {
        return None;
    }
    let name = parts.next()?.to_string();
    if name.is_empty() {
        return None;
    }
    u16::from_str_radix(addr, 16).ok().map(|addr| (addr, name))
}

fn required_value(
    iter: &mut impl Iterator<Item = String>,
    option: &str,
) -> Result<String, SimCliError> {
    iter.next()
        .ok_or_else(|| SimCliError::Message(format!("{option} requires a value")))
}

fn required_path(
    iter: &mut impl Iterator<Item = String>,
    option: &str,
) -> Result<PathBuf, SimCliError> {
    required_value(iter, option).map(PathBuf::from)
}

impl Display for SimCliError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Message(message) => formatter.write_str(message),
            Self::Io {
                action,
                path,
                error,
            } => {
                write!(
                    formatter,
                    "failed to {action} `{}`: {error}",
                    path.display()
                )
            }
            Self::InvalidMapFile { path, message } => {
                write!(
                    formatter,
                    "invalid map file `{}`: {message}",
                    path.display()
                )
            }
            Self::UnknownSymbol(symbol) => write!(formatter, "unknown symbol `{symbol}`"),
            Self::Sim(error) => Display::fmt(error, formatter),
        }
    }
}

impl std::error::Error for SimCliError {}
