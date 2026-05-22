// SPDX-License-Identifier: GPL-3.0-or-later

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::{Display, Formatter, Write};

use crate::backend::pic16::devices::{MemoryRange, TargetDevice};
use crate::common::integer::{
    compare_rel, eval_binary, eval_unary, high_byte, low_byte, normalize_value, signed_value,
    value_byte,
};
use crate::diagnostics::{Diagnostic, DiagnosticBag, Severity};
use crate::frontend::ast::{BinaryOp, UnaryOp};
use crate::frontend::semantic::{
    FunctionPointerDispatchGroup, Symbol, SymbolId, SymbolKind, TypedExpr, TypedExprKind,
    TypedGlobalInitializer, TypedProgram,
};
use crate::frontend::types::{CastKind, ScalarType, StorageClass, Type};
use crate::ir::model::{IrCondition, IrFunction, IrInstr, IrProgram, IrTerminator, Operand};
use crate::linker::map::MapFile;

use super::asm::{AsmInstr, AsmLine, AsmProgram, Dest, PeepholeStats};
use super::encoder::{LinkerRelaxationStats, encode_program, relax_page_setup};
use super::runtime::{
    MathProfile, RuntimeHelper, RuntimeHelperCategory, RuntimeHelperInfo, RuntimeProfile,
    binary_helper, runtime_helper_by_label, validate_helper_dependency_graph,
};

const STATUS_ADDR: u16 = 0x03;
const STATUS_C_BIT: u8 = 0;
const STATUS_Z_BIT: u8 = 2;
const STATUS_IRP_BIT: u8 = 7;
const INDF_ADDR: u16 = 0x00;
const FSR_ADDR: u16 = 0x04;
const PCLATH_ADDR: u16 = 0x0A;
const UNKNOWN_BANK: u8 = u8::MAX;

#[derive(Debug)]
pub struct BackendOutput {
    pub program: AsmProgram,
    pub words: BTreeMap<u16, u16>,
    pub map: MapFile,
    pub optimization: BackendOptimizationReport,
    pub stack_report: StackReport,
    pub resource_report: ResourceReport,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct BackendOptimizationReport {
    pub peephole: PeepholeStats,
    pub helper_calls_avoided: usize,
    pub relaxation: LinkerRelaxationStats,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct BackendOptions {
    pub stack_check: bool,
    pub enforce_resource_limits: bool,
    pub runtime_profile: RuntimeProfile,
    pub math_profile: MathProfile,
}

#[derive(Clone, Debug)]
pub struct StackReport {
    pub summary: StackReportSummary,
    pub text: String,
}

#[derive(Clone, Debug)]
pub struct ResourceReport {
    pub summary: ResourceSummary,
    pub size_text: String,
    pub text: String,
    pub map_lines: Vec<String>,
    pub page_layout: [PageLayoutSummary; 4],
    pub code_sections: Vec<CodeSectionSummary>,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ResourceSummary {
    pub program_words_used: u16,
    pub program_words_available: u16,
    pub highest_program_word: u16,
    pub data_ram_used: u16,
    pub modeled_data_ram_available: u16,
    pub device_data_ram_available: u16,
    pub static_data_bytes: u16,
    pub abi_slot_bytes: u16,
    pub isr_context_bytes: u16,
    pub stack_capacity: u16,
    pub estimated_max_stack: u16,
    pub rom_table_words: u16,
    pub helpers_included: u16,
    pub runtime_helper_words: u16,
    pub integer_helper_words: u16,
    pub fixed_helper_words: u16,
    pub float_helper_words: u16,
    pub math_helper_words: u16,
    pub conversion_helper_words: u16,
    pub shift_helper_words: u16,
    pub division_helper_words: u16,
    pub dispatcher_words: u16,
    pub function_pointer_dispatchers: u16,
    pub unknown_function_pointer_target_sets: u16,
    pub page_setup_removed: u16,
    pub page_relaxation_passes: u16,
    pub runtime_profile: RuntimeProfile,
    pub math_profile: MathProfile,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct PageLayoutSummary {
    pub page: u8,
    pub start: u16,
    pub end: u16,
    pub used_words: u16,
    pub free_words: u16,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CodeSectionSummary {
    pub name: String,
    pub kind: String,
    pub start: u16,
    pub end: u16,
    pub words: u16,
    pub page_start: u8,
    pub page_end: u8,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct StackReportSummary {
    pub stack_base: u16,
    pub stack_limit: u16,
    pub stack_capacity: u16,
    pub static_max_stack: u16,
    pub max_call_depth: u16,
    pub isr_frame_bytes: u16,
    pub isr_context_bytes: u16,
    pub function_pointer_groups: u16,
    pub function_pointer_targets: u16,
    pub unknown_function_pointer_target_sets: u16,
    pub stack_check: bool,
}

#[derive(Clone, Copy, Debug)]
struct RegisterPair {
    lo: u16,
    hi: u16,
}

#[derive(Clone, Copy, Debug)]
struct HelperRegisters {
    stack_ptr: RegisterPair,
    frame_ptr: RegisterPair,
    return_high: u16,
    return_upper0: u16,
    return_upper1: u16,
    scratch0: u16,
    scratch1: u16,
    flag_save: u16,
    w_save: u16,
}

#[derive(Clone, Copy, Debug)]
struct InterruptContext {
    w: u16,
    status: u16,
    pclath: u16,
    fsr: u16,
    return_high: u16,
    return_upper0: u16,
    return_upper1: u16,
    scratch0: u16,
    scratch1: u16,
    flag_save: u16,
    w_save: u16,
    stack_ptr: RegisterPair,
    frame_ptr: RegisterPair,
}

#[derive(Clone, Copy, Debug)]
struct BranchTargets<'a> {
    then_label: &'a str,
    else_label: &'a str,
}

#[derive(Debug)]
struct StorageLayout {
    helpers: HelperRegisters,
    interrupt: Option<InterruptContext>,
    symbol_storage: BTreeMap<SymbolId, SymbolStorage>,
    temp_offsets: BTreeMap<(SymbolId, usize), u16>,
    frames: BTreeMap<SymbolId, FrameLayout>,
    stack_base: u16,
    stack_end: u16,
    stack_limit: u16,
    stack_capacity: u16,
    max_stack_depth: u16,
}

#[derive(Clone, Copy, Debug)]
enum SymbolStorage {
    Absolute(u16),
    Frame(u16),
}

#[derive(Clone, Debug)]
struct FrameLayout {
    arg_bytes: u16,
    saved_fp_offset: u16,
    local_bytes: u16,
    temp_bytes: u16,
    frame_bytes: u16,
}

#[derive(Clone, Copy, Debug)]
struct RomTablePlacement {
    symbol: SymbolId,
    start: u16,
}

#[derive(Clone, Debug)]
struct StackFunctionReport {
    symbol: SymbolId,
    frame: FrameLayout,
    helper_extra: u16,
    max_stack_bytes: u16,
    max_call_depth: u16,
    direct_callees: Vec<SymbolId>,
    indirect_groups: Vec<IndirectTargetSet>,
}

#[derive(Clone, Debug)]
struct IndirectTargetSet {
    signature: Type,
    targets: Vec<SymbolId>,
    unknown: bool,
}

#[derive(Clone, Debug)]
struct StackAnalysis {
    functions: Vec<StackFunctionReport>,
    summary: StackReportSummary,
}

#[derive(Clone, Debug)]
struct ResourceContribution {
    name: String,
    kind: ResourceContributionKind,
    start: u16,
    words: u16,
    stack_frame: Option<u16>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ResourceContributionKind {
    VectorStartup,
    Function,
    RuntimeHelper,
    FixedPointHelper,
    FloatHelper,
    MathHelper,
    ShiftHelper,
    FunctionPointerDispatcher,
    RomTable,
    Internal,
}

impl Display for ResourceContributionKind {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        let text = match self {
            Self::VectorStartup => "vectors/startup",
            Self::Function => "function",
            Self::RuntimeHelper => "runtime helper",
            Self::FixedPointHelper => "fixed-point helper",
            Self::FloatHelper => "float helper",
            Self::MathHelper => "math helper",
            Self::ShiftHelper => "shift helper",
            Self::FunctionPointerDispatcher => "function pointer dispatcher",
            Self::RomTable => "ROM RETLW table",
            Self::Internal => "internal",
        };
        formatter.write_str(text)
    }
}

fn program_needs_32bit_return_slots(typed_program: &TypedProgram, ir_program: &IrProgram) -> bool {
    typed_program.symbols.iter().any(|symbol| {
        type_has_32bit_scalar(symbol.ty)
            || symbol
                .parameter_types
                .iter()
                .copied()
                .any(type_has_32bit_scalar)
    }) || typed_program
        .functions
        .iter()
        .any(|function| type_has_32bit_scalar(function.return_type))
        || ir_program.functions.iter().any(|function| {
            type_has_32bit_scalar(function.return_type)
                || function
                    .temp_types
                    .iter()
                    .copied()
                    .any(type_has_32bit_scalar)
        })
}

fn type_has_32bit_scalar(ty: Type) -> bool {
    if ty.is_function_pointer() {
        return matches!(
            ty.function_return_scalar(),
            Some(
                ScalarType::I32
                    | ScalarType::U32
                    | ScalarType::F32
                    | ScalarType::Q16_16
                    | ScalarType::UQ16_16,
            )
        ) || (0..ty.function_param_len().unwrap_or(0)).any(|index| {
            matches!(
                ty.function_param_scalar(index),
                Some(
                    ScalarType::I32
                        | ScalarType::U32
                        | ScalarType::F32
                        | ScalarType::Q16_16
                        | ScalarType::UQ16_16,
                )
            )
        });
    }
    if ty.is_array() {
        return type_has_32bit_scalar(ty.element_type());
    }
    ty.pointer_depth == 0
        && matches!(
            ty.scalar,
            ScalarType::I32
                | ScalarType::U32
                | ScalarType::F32
                | ScalarType::Q16_16
                | ScalarType::UQ16_16
        )
}

/// Lowers typed IR into assembly, encoded words, and a final linker map.
pub fn compile_program(
    target: &TargetDevice,
    typed_program: &TypedProgram,
    ir_program: &IrProgram,
    options: &BackendOptions,
    diagnostics: &mut DiagnosticBag,
) -> Option<BackendOutput> {
    let layout = StorageAllocator::new(target.allocatable_gpr, target.shared_gpr).layout(
        typed_program,
        ir_program,
        diagnostics,
    )?;
    let stack_analysis = analyze_stack(typed_program, ir_program, &layout, *options);

    let mut codegen = CodegenContext::new(target, typed_program, &layout, *options);
    codegen.emit_program(ir_program, diagnostics);
    if diagnostics.has_errors() {
        return None;
    }

    let mut optimization = codegen.optimize_program();
    codegen.emit_stack_overflow_trap();
    codegen.emit_rom_objects(diagnostics);
    if diagnostics.has_errors() {
        return None;
    }
    optimization.relaxation = relax_page_setup(&mut codegen.program, diagnostics);
    if diagnostics.has_errors() {
        return None;
    }

    let encoded = encode_program(&codegen.program, diagnostics)?;
    let stack_report = StackReport {
        summary: stack_analysis.summary,
        text: render_stack_report(target, typed_program, &layout, &stack_analysis, *options),
    };
    let resource_report = build_resource_report(ResourceReportInputs {
        target,
        typed_program,
        layout: &layout,
        words: &encoded.words,
        labels: &encoded.labels,
        stack: &stack_report.summary,
        relaxation: optimization.relaxation,
        runtime_profile: options.runtime_profile,
        math_profile: options.math_profile,
    });
    validate_resource_fit(
        target,
        &resource_report,
        &encoded.words,
        *options,
        diagnostics,
    );
    if diagnostics.has_errors() {
        return None;
    }
    let map = build_map(
        typed_program,
        &layout,
        &encoded.labels,
        resource_report.map_lines.clone(),
        &resource_report.code_sections,
        &resource_report.page_layout,
    );
    Some(BackendOutput {
        program: codegen.program,
        words: encoded.words,
        map,
        optimization,
        stack_report,
        resource_report,
    })
}

struct StorageAllocator<'a> {
    ranges: &'a [MemoryRange],
    shared_ranges: &'a [MemoryRange],
}

impl<'a> StorageAllocator<'a> {
    /// Creates a RAM allocator over the device's allocatable GPR ranges.
    fn new(ranges: &'a [MemoryRange], shared_ranges: &'a [MemoryRange]) -> Self {
        Self {
            ranges,
            shared_ranges,
        }
    }

    /// Assigns RAM slots for globals, per-frame autos, and backend helper storage.
    fn layout(
        &self,
        typed_program: &TypedProgram,
        ir_program: &IrProgram,
        diagnostics: &mut DiagnosticBag,
    ) -> Option<StorageLayout> {
        let mut allocator = AddressAllocator::new(self.ranges);
        let needs_32bit_return_slots = program_needs_32bit_return_slots(typed_program, ir_program);

        let Some(stack_ptr_lo) = allocator.next_span(2) else {
            diagnostics.error("backend", None, "not enough RAM for ABI helper slots", None);
            return None;
        };
        let Some(frame_ptr_lo) = allocator.next_span(2) else {
            diagnostics.error("backend", None, "not enough RAM for ABI helper slots", None);
            return None;
        };
        let Some(return_high) = allocator.next_span(1) else {
            diagnostics.error("backend", None, "not enough RAM for ABI helper slots", None);
            return None;
        };
        let return_upper0 = if needs_32bit_return_slots {
            let Some(slot) = allocator.next_span(1) else {
                diagnostics.error("backend", None, "not enough RAM for ABI helper slots", None);
                return None;
            };
            slot
        } else {
            return_high
        };
        let return_upper1 = if needs_32bit_return_slots {
            let Some(slot) = allocator.next_span(1) else {
                diagnostics.error("backend", None, "not enough RAM for ABI helper slots", None);
                return None;
            };
            slot
        } else {
            return_high
        };
        let Some(scratch0) = allocator.next_span(1) else {
            diagnostics.error("backend", None, "not enough RAM for ABI helper slots", None);
            return None;
        };
        let Some(scratch1) = allocator.next_span(1) else {
            diagnostics.error("backend", None, "not enough RAM for ABI helper slots", None);
            return None;
        };
        let Some(flag_save) = allocator.next_span(1) else {
            diagnostics.error("backend", None, "not enough RAM for ABI helper slots", None);
            return None;
        };
        let Some(w_save) = allocator.next_span(1) else {
            diagnostics.error("backend", None, "not enough RAM for ABI helper slots", None);
            return None;
        };

        let helpers = HelperRegisters {
            stack_ptr: RegisterPair {
                lo: stack_ptr_lo,
                hi: stack_ptr_lo + 1,
            },
            frame_ptr: RegisterPair {
                lo: frame_ptr_lo,
                hi: frame_ptr_lo + 1,
            },
            return_high,
            return_upper0,
            return_upper1,
            scratch0,
            scratch1,
            flag_save,
            w_save,
        };

        let interrupt = if ir_program
            .functions
            .iter()
            .any(|function| function.is_interrupt)
        {
            let mut shared = AddressAllocator::new(self.shared_ranges);
            let Some(w) = shared.next_span(1) else {
                diagnostics.error(
                    "backend",
                    None,
                    "not enough shared RAM for ISR context",
                    None,
                );
                return None;
            };
            let Some(status) = shared.next_span(1) else {
                diagnostics.error(
                    "backend",
                    None,
                    "not enough shared RAM for ISR context",
                    None,
                );
                return None;
            };
            let Some(pclath) = shared.next_span(1) else {
                diagnostics.error(
                    "backend",
                    None,
                    "not enough shared RAM for ISR context",
                    None,
                );
                return None;
            };
            let Some(fsr) = shared.next_span(1) else {
                diagnostics.error(
                    "backend",
                    None,
                    "not enough shared RAM for ISR context",
                    None,
                );
                return None;
            };
            let Some(return_high_ctx) = shared.next_span(1) else {
                diagnostics.error(
                    "backend",
                    None,
                    "not enough shared RAM for ISR context",
                    None,
                );
                return None;
            };
            let return_upper0_ctx = if needs_32bit_return_slots {
                let Some(slot) = shared.next_span(1) else {
                    diagnostics.error(
                        "backend",
                        None,
                        "not enough shared RAM for ISR context",
                        None,
                    );
                    return None;
                };
                slot
            } else {
                return_high_ctx
            };
            let return_upper1_ctx = if needs_32bit_return_slots {
                let Some(slot) = shared.next_span(1) else {
                    diagnostics.error(
                        "backend",
                        None,
                        "not enough shared RAM for ISR context",
                        None,
                    );
                    return None;
                };
                slot
            } else {
                return_high_ctx
            };
            let Some(scratch0_ctx) = shared.next_span(1) else {
                diagnostics.error(
                    "backend",
                    None,
                    "not enough shared RAM for ISR context",
                    None,
                );
                return None;
            };
            let Some(scratch1_ctx) = shared.next_span(1) else {
                diagnostics.error(
                    "backend",
                    None,
                    "not enough shared RAM for ISR context",
                    None,
                );
                return None;
            };
            let Some(flag_save_ctx) = shared.next_span(1) else {
                diagnostics.error(
                    "backend",
                    None,
                    "not enough shared RAM for ISR context",
                    None,
                );
                return None;
            };
            let Some(w_save_ctx) = shared.next_span(1) else {
                diagnostics.error(
                    "backend",
                    None,
                    "not enough shared RAM for ISR context",
                    None,
                );
                return None;
            };
            let Some(stack_ptr_ctx_lo) = shared.next_span(2) else {
                diagnostics.error(
                    "backend",
                    None,
                    "not enough shared RAM for ISR context",
                    None,
                );
                return None;
            };
            let Some(frame_ptr_ctx_lo) = shared.next_span(2) else {
                diagnostics.error(
                    "backend",
                    None,
                    "not enough shared RAM for ISR context",
                    None,
                );
                return None;
            };

            Some(InterruptContext {
                w,
                status,
                pclath,
                fsr,
                return_high: return_high_ctx,
                return_upper0: return_upper0_ctx,
                return_upper1: return_upper1_ctx,
                scratch0: scratch0_ctx,
                scratch1: scratch1_ctx,
                flag_save: flag_save_ctx,
                w_save: w_save_ctx,
                stack_ptr: RegisterPair {
                    lo: stack_ptr_ctx_lo,
                    hi: stack_ptr_ctx_lo + 1,
                },
                frame_ptr: RegisterPair {
                    lo: frame_ptr_ctx_lo,
                    hi: frame_ptr_ctx_lo + 1,
                },
            })
        } else {
            None
        };

        let mut symbol_storage = BTreeMap::new();
        for symbol in &typed_program.symbols {
            if let Some(addr) = symbol.fixed_address {
                symbol_storage.insert(symbol.id, SymbolStorage::Absolute(addr));
                continue;
            }
            if symbol.kind == SymbolKind::Global
                || symbol.kind == SymbolKind::StringLiteral
                || (symbol.kind == SymbolKind::Local
                    && symbol.storage_class == crate::frontend::types::StorageClass::Static)
            {
                if symbol.ty.is_rom() {
                    continue;
                }
                let Some(base) = allocator.next_span(symbol.ty.byte_width()) else {
                    diagnostics.error(
                        "backend",
                        None,
                        format!(
                            "data RAM overflow: not enough allocatable RAM for symbol `{}`",
                            symbol.name
                        ),
                        None,
                    );
                    return None;
                };
                symbol_storage.insert(symbol.id, SymbolStorage::Absolute(base));
            }
        }

        let ir_functions = ir_program
            .functions
            .iter()
            .map(|function| (function.symbol, function))
            .collect::<BTreeMap<_, _>>();
        let mut frames = BTreeMap::new();
        let mut temp_offsets = BTreeMap::new();
        for function in &typed_program.functions {
            let mut arg_bytes = 0u16;
            for param in &function.params {
                symbol_storage.insert(*param, SymbolStorage::Frame(arg_bytes));
                arg_bytes += self.symbol_width(typed_program, *param)?;
            }

            let saved_fp_offset = arg_bytes;
            let mut local_cursor = arg_bytes + 2;
            for local in &function.locals {
                let symbol = &typed_program.symbols[*local];
                if symbol.kind != SymbolKind::Local
                    || symbol.storage_class == crate::frontend::types::StorageClass::Static
                {
                    continue;
                }
                symbol_storage.insert(*local, SymbolStorage::Frame(local_cursor));
                local_cursor += self.symbol_width(typed_program, *local)?;
            }
            let local_bytes = local_cursor - (arg_bytes + 2);

            let mut temp_cursor = local_cursor;
            if let Some(ir_function) = ir_functions.get(&function.symbol) {
                for (temp, ty) in ir_function.temp_types.iter().enumerate() {
                    temp_offsets.insert((function.symbol, temp), temp_cursor);
                    temp_cursor += u16::try_from(ty.byte_width()).ok()?;
                }
            }
            let temp_bytes = temp_cursor - local_cursor;

            frames.insert(
                function.symbol,
                FrameLayout {
                    arg_bytes,
                    saved_fp_offset,
                    local_bytes,
                    temp_bytes,
                    frame_bytes: temp_cursor - arg_bytes,
                },
            );
        }

        let Some((stack_base, stack_end, stack_capacity)) = allocator.stack_region() else {
            diagnostics.error(
                "backend",
                None,
                "stack region overflow: not enough RAM left for the Phase 4 software stack",
                None,
            );
            return None;
        };

        let max_stack_depth =
            compute_max_stack_depth_with_interrupts(typed_program, ir_program, &frames);
        if max_stack_depth > stack_capacity {
            diagnostics.error(
                "backend",
                None,
                format!(
                    "stack region overflow: Phase 4 software stack needs {max_stack_depth} bytes but only {stack_capacity} bytes remain"
                ),
                Some("reduce local storage, call depth, or argument count for this target".to_string()),
            );
            return None;
        }

        Some(StorageLayout {
            helpers,
            interrupt,
            symbol_storage,
            temp_offsets,
            frames,
            stack_base,
            stack_end,
            stack_limit: stack_end + 1,
            stack_capacity,
            max_stack_depth,
        })
    }

    /// Returns the byte width of one symbol object while validating the conversion to `u16`.
    fn symbol_width(&self, typed_program: &TypedProgram, symbol: SymbolId) -> Option<u16> {
        u16::try_from(typed_program.symbols[symbol].ty.byte_width()).ok()
    }
}

struct AddressAllocator<'a> {
    ranges: &'a [MemoryRange],
    range_index: usize,
    next_addr: u16,
}

impl<'a> AddressAllocator<'a> {
    /// Creates a range-walking allocator positioned at the first RAM range.
    fn new(ranges: &'a [MemoryRange]) -> Self {
        Self {
            ranges,
            range_index: 0,
            next_addr: ranges.first().map_or(0, |range| range.start),
        }
    }

    /// Allocates one contiguous byte span from the configured RAM ranges.
    fn next_span(&mut self, width: usize) -> Option<u16> {
        let width = u16::try_from(width).ok()?;
        while let Some(range) = self.ranges.get(self.range_index).copied() {
            if self.next_addr > range.end {
                self.range_index += 1;
                if let Some(next) = self.ranges.get(self.range_index) {
                    self.next_addr = next.start;
                }
                continue;
            }
            let end = self.next_addr + width.saturating_sub(1);
            if end <= range.end {
                let base = self.next_addr;
                self.next_addr = end + 1;
                return Some(base);
            }
            self.range_index += 1;
            if let Some(next) = self.ranges.get(self.range_index) {
                self.next_addr = next.start;
            }
        }
        None
    }

    /// Returns the remaining contiguous tail region reserved for the software stack.
    fn stack_region(&self) -> Option<(u16, u16, u16)> {
        let range = self.ranges.get(self.range_index).copied()?;
        if self.next_addr > range.end {
            return None;
        }
        Some((self.next_addr, range.end, range.end - self.next_addr + 1))
    }
}

struct CodegenContext<'a> {
    target: &'a TargetDevice,
    typed_program: &'a TypedProgram,
    layout: &'a StorageLayout,
    options: BackendOptions,
    program: AsmProgram,
    current_bank: u8,
    label_counter: usize,
    used_helpers: BTreeSet<RuntimeHelper>,
    helper_calls_avoided: usize,
}

impl<'a> CodegenContext<'a> {
    /// Creates a backend codegen context for one target, program, and storage layout.
    fn new(
        target: &'a TargetDevice,
        typed_program: &'a TypedProgram,
        layout: &'a StorageLayout,
        options: BackendOptions,
    ) -> Self {
        Self {
            target,
            typed_program,
            layout,
            options,
            program: AsmProgram::new(),
            current_bank: UNKNOWN_BANK,
            label_counter: 0,
            used_helpers: BTreeSet::new(),
            helper_calls_avoided: 0,
        }
    }

    /// Emits vectors, startup, and all reachable function bodies for the IR program.
    fn emit_program(&mut self, ir_program: &IrProgram, diagnostics: &mut DiagnosticBag) {
        self.emit_vectors();
        self.emit_startup(ir_program, diagnostics);
        for function in &ir_program.functions {
            self.emit_function(function, diagnostics);
        }
        self.emit_function_pointer_dispatchers();
        self.emit_runtime_helpers(diagnostics);
    }

    /// Applies backend-local optimization passes and returns a summary for reporting.
    fn optimize_program(&mut self) -> BackendOptimizationReport {
        BackendOptimizationReport {
            peephole: self.program.peephole_optimize(),
            helper_calls_avoided: self.helper_calls_avoided,
            relaxation: LinkerRelaxationStats::default(),
        }
    }

    /// Emits reset and interrupt vector stubs for the current target descriptor.
    fn emit_vectors(&mut self) {
        let interrupt = self
            .typed_program
            .symbols
            .iter()
            .find(|symbol| symbol.kind == SymbolKind::Function && symbol.is_interrupt)
            .map(|symbol| function_label(&symbol.name));
        self.program.push(AsmLine::Org(self.target.vectors.reset));
        self.program
            .push(AsmLine::Label("__reset_vector".to_string()));
        self.program.push(AsmLine::Instr(AsmInstr::Goto(
            "__reset_dispatch".to_string(),
        )));
        self.program
            .push(AsmLine::Org(self.target.vectors.interrupt));
        self.program
            .push(AsmLine::Label("__interrupt_vector".to_string()));
        if interrupt.is_some() {
            self.program.push(AsmLine::Instr(AsmInstr::Goto(
                "__interrupt_dispatch".to_string(),
            )));
        } else {
            self.program.push(AsmLine::Instr(AsmInstr::Retfie));
        }
        self.program
            .push(AsmLine::Org(self.target.vectors.interrupt + 1));
        self.program
            .push(AsmLine::Label("__reset_dispatch".to_string()));
        self.program
            .push(AsmLine::Instr(AsmInstr::SetPage("__start".to_string())));
        self.program
            .push(AsmLine::Instr(AsmInstr::Goto("__start".to_string())));
        if let Some(interrupt) = interrupt {
            self.program
                .push(AsmLine::Label("__interrupt_dispatch".to_string()));
            self.program
                .push(AsmLine::Instr(AsmInstr::SetPage(interrupt.clone())));
            self.program.push(AsmLine::Instr(AsmInstr::Goto(interrupt)));
        }
        self.program.push(AsmLine::Label("__start".to_string()));
    }

    /// Emits startup initialization for globals and transfers control to `main`.
    fn emit_startup(&mut self, ir_program: &IrProgram, diagnostics: &mut DiagnosticBag) {
        self.program.push(AsmLine::Comment(format!(
            "stack base=0x{:04X} end=0x{:04X} limit=0x{:04X} capacity={} max_depth={} stack_check={}",
            self.layout.stack_base,
            self.layout.stack_end,
            self.layout.stack_limit,
            self.layout.stack_capacity,
            self.layout.max_stack_depth,
            if self.options.stack_check { "on" } else { "off" }
        )));
        self.program
            .push(AsmLine::Comment("static data initialization".to_string()));
        for global in &self.typed_program.globals {
            let symbol = &self.typed_program.symbols[global.symbol];
            let Some(SymbolStorage::Absolute(base)) =
                self.layout.symbol_storage.get(&global.symbol).copied()
            else {
                continue;
            };
            let symbol_name = format_data_symbol_name(symbol);
            if let Some(initializer) = &global.initializer {
                match initializer {
                    TypedGlobalInitializer::Scalar(initializer) => {
                        self.program.push(AsmLine::Comment(format!(
                            "init {symbol_name} @0x{base:04X} ({} byte scalar)",
                            symbol.ty.byte_width()
                        )));
                        let value = eval_const_expr(initializer);
                        self.store_const_value(base, symbol.ty, value);
                    }
                    TypedGlobalInitializer::Bytes(bytes) => {
                        self.program.push(AsmLine::Comment(format!(
                            "init {symbol_name} @0x{base:04X} ({} byte payload)",
                            bytes.len()
                        )));
                        for (index, byte) in bytes.iter().enumerate() {
                            self.emit_const_to_w(*byte);
                            self.store_w_to_addr(base + index as u16);
                        }
                    }
                    TypedGlobalInitializer::Address { symbol, offset } => {
                        let Some(SymbolStorage::Absolute(address_base)) =
                            self.layout.symbol_storage.get(symbol).copied()
                        else {
                            diagnostics.error(
                                "backend",
                                None,
                                "static pointer initializer references a non-static address",
                                None,
                            );
                            continue;
                        };
                        let address = address_base + *offset as u16;
                        self.program.push(AsmLine::Comment(format!(
                            "init {symbol_name} @0x{base:04X} (address of {} + {offset})",
                            format_data_symbol_name(&self.typed_program.symbols[*symbol])
                        )));
                        self.store_const_value(
                            base,
                            Type::new(ScalarType::U16),
                            i64::from(address),
                        );
                    }
                }
            } else {
                self.program.push(AsmLine::Comment(format!(
                    "zero {symbol_name} @0x{base:04X} ({} bytes)",
                    symbol.ty.byte_width()
                )));
                self.clear_slot(base, symbol.ty);
            }
        }

        self.store_const_value(
            self.layout.helpers.stack_ptr.lo,
            Type::new(ScalarType::U16),
            i64::from(self.layout.stack_base),
        );
        self.store_const_value(
            self.layout.helpers.frame_ptr.lo,
            Type::new(ScalarType::U16),
            i64::from(self.layout.stack_base),
        );

        let Some(main_symbol) = self
            .typed_program
            .symbols
            .iter()
            .find(|symbol| symbol.kind == SymbolKind::Function && symbol.name == "main")
            .map(|symbol| symbol.id)
        else {
            diagnostics.error("backend", None, "entry function `main` not found", None);
            return;
        };

        if let Some(function) = ir_program
            .functions
            .iter()
            .find(|function| function.symbol == main_symbol)
            && !function.params.is_empty()
        {
            let _ = function;
            diagnostics.error(
                "backend",
                None,
                "phase 4 requires `main` with no parameters",
                None,
            );
        }

        let label = function_label(self.symbol_name(main_symbol));
        self.program
            .push(AsmLine::Instr(AsmInstr::SetPage(label.clone())));
        self.program.push(AsmLine::Instr(AsmInstr::Call(label)));
        self.restore_code_page_after_call();
        self.program.push(AsmLine::Label("__halt".to_string()));
        self.program
            .push(AsmLine::Instr(AsmInstr::SetPage("__halt".to_string())));
        self.program
            .push(AsmLine::Instr(AsmInstr::Goto("__halt".to_string())));
    }

    /// Emits one function body and records the per-call frame layout in assembly comments.
    fn emit_function(&mut self, function: &IrFunction, diagnostics: &mut DiagnosticBag) {
        let name = self.symbol_name(function.symbol).to_string();
        let frame = self.frame_layout(function.symbol);
        let arg_bytes = frame.arg_bytes;
        let saved_fp_offset = frame.saved_fp_offset;
        let local_bytes = frame.local_bytes;
        let temp_bytes = frame.temp_bytes;
        let frame_bytes = frame.frame_bytes;
        self.program.push(AsmLine::Label(function_label(&name)));
        self.program.push(AsmLine::Comment(format!(
            "frame args={} saved_fp={} locals={} temps={} frame_bytes={}",
            arg_bytes, saved_fp_offset, local_bytes, temp_bytes, frame_bytes
        )));
        self.current_bank = UNKNOWN_BANK;
        if function.is_interrupt {
            self.program.push(AsmLine::Comment(
                "interrupt context save + isolated stack frame".to_string(),
            ));
            self.emit_interrupt_prologue(function.symbol);
        } else {
            self.emit_prologue(function.symbol);
        }

        let reachable = function.reachable_blocks();
        for block in &function.blocks {
            if !reachable.contains(&block.id) {
                continue;
            }
            self.program
                .push(AsmLine::Label(block_label(&name, block.id)));
            for instr in &block.instructions {
                self.emit_instr(function, instr, diagnostics);
            }
            self.emit_terminator(function, &block.terminator, diagnostics);
        }
    }

    /// Lowers a single IR instruction into PIC16 assembly.
    fn emit_instr(
        &mut self,
        function: &IrFunction,
        instr: &IrInstr,
        diagnostics: &mut DiagnosticBag,
    ) {
        match instr {
            IrInstr::Copy { dst, src } => {
                let ty = function.temp_types[*dst];
                self.copy_operand_to_temp(function.symbol, *src, ty, *dst);
            }
            IrInstr::AddrOf { dst, symbol } => {
                let dst_ty = function.temp_types[*dst];
                self.emit_address_of_symbol(function.symbol, *symbol, dst_ty, *dst);
            }
            IrInstr::Cast {
                dst,
                kind,
                src,
                src_ty,
            } => {
                let dst_ty = function.temp_types[*dst];
                self.emit_cast(function.symbol, *src, *src_ty, *kind, dst_ty, *dst);
            }
            IrInstr::Unary { dst, op, src } => {
                let dst_ty = function.temp_types[*dst];
                match op {
                    UnaryOp::Negate => self.emit_negate(function.symbol, *src, dst_ty, *dst),
                    UnaryOp::BitwiseNot => {
                        self.emit_per_byte_unary(
                            function.symbol,
                            *src,
                            dst_ty,
                            *dst,
                            |this, sym, operand, ty, byte| {
                                this.load_operand_byte_to_w(sym, operand, ty, byte);
                                this.program.push(AsmLine::Instr(AsmInstr::Xorlw(0xFF)));
                            },
                        );
                    }
                    UnaryOp::LogicalNot => {
                        diagnostics.error(
                            "backend",
                            None,
                            "logical not should lower through branch form before backend",
                            None,
                        );
                        self.clear_temp(function.symbol, *dst, dst_ty);
                    }
                }
            }
            IrInstr::Binary { dst, op, lhs, rhs } => {
                let dst_ty = function.temp_types[*dst];
                match op {
                    BinaryOp::Add if dst_ty.is_float() => self.emit_runtime_binary_call(
                        function,
                        *op,
                        *lhs,
                        *rhs,
                        dst_ty,
                        *dst,
                        diagnostics,
                    ),
                    BinaryOp::Add => self.emit_add(function.symbol, *lhs, *rhs, dst_ty, *dst),
                    BinaryOp::Sub if dst_ty.is_float() => self.emit_runtime_binary_call(
                        function,
                        *op,
                        *lhs,
                        *rhs,
                        dst_ty,
                        *dst,
                        diagnostics,
                    ),
                    BinaryOp::Sub => self.emit_sub(function.symbol, *lhs, *rhs, dst_ty, *dst),
                    BinaryOp::BitAnd => {
                        self.emit_per_byte_binary(
                            function.symbol,
                            *lhs,
                            *rhs,
                            dst_ty,
                            *dst,
                            |_this, f| AsmInstr::Andwf { f, d: Dest::W },
                        );
                    }
                    BinaryOp::BitOr => {
                        self.emit_per_byte_binary(
                            function.symbol,
                            *lhs,
                            *rhs,
                            dst_ty,
                            *dst,
                            |this, f| {
                                let _ = this;
                                AsmInstr::Iorwf { f, d: Dest::W }
                            },
                        );
                    }
                    BinaryOp::BitXor => {
                        self.emit_per_byte_binary(
                            function.symbol,
                            *lhs,
                            *rhs,
                            dst_ty,
                            *dst,
                            |this, f| {
                                let _ = this;
                                AsmInstr::Xorwf { f, d: Dest::W }
                            },
                        );
                    }
                    BinaryOp::Equal
                    | BinaryOp::NotEqual
                    | BinaryOp::Less
                    | BinaryOp::LessEqual
                    | BinaryOp::Greater
                    | BinaryOp::GreaterEqual
                    | BinaryOp::LogicalAnd
                    | BinaryOp::LogicalOr => {
                        diagnostics.error(
                            "backend",
                            None,
                            format!("IR should lower `{op:?}` into branch form before backend"),
                            None,
                        );
                        self.clear_temp(function.symbol, *dst, dst_ty);
                    }
                    BinaryOp::Multiply | BinaryOp::Divide | BinaryOp::Modulo => {
                        self.emit_phase_five_binary(
                            function,
                            *op,
                            *lhs,
                            *rhs,
                            dst_ty,
                            *dst,
                            diagnostics,
                        );
                    }
                    BinaryOp::ShiftLeft | BinaryOp::ShiftRight => {
                        self.emit_phase_five_binary(
                            function,
                            *op,
                            *lhs,
                            *rhs,
                            dst_ty,
                            *dst,
                            diagnostics,
                        );
                    }
                }
            }
            IrInstr::Store { target, value } => {
                let target_ty = self.symbol_type(*target);
                self.copy_operand_to_symbol(function.symbol, *value, target_ty, *target);
            }
            IrInstr::LoadIndirect { dst, ptr } => {
                let dst_ty = function.temp_types[*dst];
                self.emit_indirect_load(function.symbol, *ptr, dst_ty, *dst);
            }
            IrInstr::StoreIndirect { ptr, value, ty } => {
                self.emit_indirect_store(function.symbol, *ptr, *value, *ty);
            }
            IrInstr::RomRead8 { dst, symbol, index } => {
                self.emit_rom_read8(function, *symbol, *index, *dst, diagnostics);
            }
            IrInstr::RomRead16 { dst, symbol, index } => {
                self.emit_rom_read16(function, *symbol, *index, *dst, diagnostics);
            }
            IrInstr::RomRead32 { dst, symbol, index } => {
                self.emit_rom_read32(function, *symbol, *index, *dst, diagnostics);
            }
            IrInstr::Call {
                dst,
                function: callee,
                args,
            } => {
                self.emit_call(function, *callee, args, *dst, diagnostics);
            }
            IrInstr::IndirectCall {
                dst,
                callee,
                signature,
                args,
            } => {
                self.emit_indirect_call(function, *callee, *signature, args, *dst, diagnostics);
            }
        }
    }

    /// Emits the control-flow terminator for the current IR block.
    fn emit_terminator(
        &mut self,
        function: &IrFunction,
        terminator: &IrTerminator,
        diagnostics: &mut DiagnosticBag,
    ) {
        let fn_name = self.symbol_name(function.symbol);
        match terminator {
            IrTerminator::Return(value) => {
                if let Some(value) = value {
                    self.emit_return_value(function.symbol, *value, function.return_type);
                }
                if function.is_interrupt {
                    self.emit_interrupt_epilogue(function.symbol);
                    self.program.push(AsmLine::Instr(AsmInstr::Retfie));
                } else {
                    self.emit_epilogue(function.symbol);
                    self.program.push(AsmLine::Instr(AsmInstr::Return));
                }
            }
            IrTerminator::Jump(target) => self.jump_to_label(&block_label(fn_name, *target)),
            IrTerminator::Branch {
                condition,
                then_block,
                else_block,
            } => {
                let then_label = block_label(fn_name, *then_block);
                let else_label = block_label(fn_name, *else_block);
                self.emit_branch(function, condition, &then_label, &else_label, diagnostics);
            }
            IrTerminator::Unreachable => {}
        }
    }

    /// Lowers a direct call using the Phase 4 software-stack ABI and captures return values.
    fn emit_call(
        &mut self,
        function: &IrFunction,
        callee: SymbolId,
        args: &[Operand],
        dst: Option<usize>,
        diagnostics: &mut DiagnosticBag,
    ) {
        let callee_name = self.symbol_name(callee).to_string();
        if let Some(helper) = runtime_helper_for_math_call(&callee_name) {
            self.emit_float_math_call(function, helper, args, dst, diagnostics);
            return;
        }

        if function.is_interrupt {
            diagnostics.error(
                "backend",
                None,
                format!(
                    "interrupt handler `{}` reached normal-call lowering for `{}`",
                    self.symbol_name(function.symbol),
                    callee_name
                ),
                Some("phase 6 forbids normal function calls inside ISRs".to_string()),
            );
            if let Some(dst) = dst {
                self.clear_temp(function.symbol, dst, function.temp_types[dst]);
            }
            return;
        }

        let callee_symbol = &self.typed_program.symbols[callee];
        let arg_bytes = self.function_arg_bytes(callee);
        self.emit_stack_growth_check(arg_bytes, &format!("call {} arguments", callee_symbol.name));
        for (index, arg) in args.iter().enumerate() {
            let Some(param_ty) = callee_symbol.parameter_types.get(index).copied() else {
                diagnostics.error(
                    "backend",
                    None,
                    "call passes more arguments than callee signature",
                    None,
                );
                continue;
            };
            self.push_operand(function.symbol, *arg, param_ty);
        }

        let label = function_label(self.symbol_name(callee));
        self.program
            .push(AsmLine::Instr(AsmInstr::SetPage(label.clone())));
        self.program.push(AsmLine::Instr(AsmInstr::Call(label)));
        self.restore_code_page_after_call();

        if dst.is_some() {
            self.store_w_to_addr(self.layout.helpers.w_save);
        }
        if arg_bytes != 0 {
            self.add_immediate_to_pair(self.layout.helpers.stack_ptr, negate_u16(arg_bytes));
        }

        if let Some(dst) = dst {
            let dst_ty = function.temp_types[dst];
            self.load_addr_to_w(self.layout.helpers.w_save);
            self.store_w_to_temp_byte(function.symbol, dst, 0);
            for byte in 1..dst_ty.byte_width() {
                self.load_return_byte_to_w(byte);
                self.store_w_to_temp_byte(function.symbol, dst, byte);
            }
        }
    }

    /// Lowers Phase 36 finite `math.h` float calls to known runtime helpers.
    fn emit_float_math_call(
        &mut self,
        function: &IrFunction,
        helper: RuntimeHelper,
        args: &[Operand],
        dst: Option<usize>,
        diagnostics: &mut DiagnosticBag,
    ) {
        let f32_ty = Type::new(ScalarType::F32);
        if args.len() != 1 {
            diagnostics.error(
                "backend",
                None,
                format!("math helper `{}` expects one argument", helper.label()),
                None,
            );
            if let Some(dst) = dst {
                self.clear_temp(function.symbol, dst, f32_ty);
            }
            return;
        }

        if function.is_interrupt {
            if helper == RuntimeHelper::F32Fabs {
                if let Some(dst) = dst {
                    self.emit_inline_fabs(function.symbol, args[0], dst);
                }
                return;
            }
            diagnostics.error(
                "backend",
                None,
                format!(
                    "interrupt handler `{}` reached float math helper lowering for `{}`",
                    self.symbol_name(function.symbol),
                    helper.label()
                ),
                Some("Phase 36 allows only inline `fabsf` inside ISRs".to_string()),
            );
            if let Some(dst) = dst {
                self.clear_temp(function.symbol, dst, f32_ty);
            }
            return;
        }

        let Some(dst) = dst else {
            return;
        };
        let info = helper.info();
        self.used_helpers.insert(helper);
        self.emit_stack_growth_check(
            info.arg_bytes,
            &format!("runtime helper {} argument", info.label),
        );
        self.push_operand(function.symbol, args[0], f32_ty);
        self.program
            .push(AsmLine::Instr(AsmInstr::SetPage(info.label.to_string())));
        self.program
            .push(AsmLine::Instr(AsmInstr::Call(info.label.to_string())));
        self.restore_code_page_after_call();
        self.store_w_to_addr(self.layout.helpers.w_save);
        self.add_immediate_to_pair(self.layout.helpers.stack_ptr, negate_u16(info.arg_bytes));
        self.load_addr_to_w(self.layout.helpers.w_save);
        self.store_w_to_temp_byte(function.symbol, dst, 0);
        for byte in 1..f32_ty.byte_width() {
            self.load_return_byte_to_w(byte);
            self.store_w_to_temp_byte(function.symbol, dst, byte);
        }
    }

    /// Emits inline `fabsf` for ISR-safe use by clearing the destination sign bit.
    fn emit_inline_fabs(&mut self, function_symbol: SymbolId, src: Operand, dst_temp: usize) {
        let f32_ty = Type::new(ScalarType::F32);
        self.copy_operand_to_temp(function_symbol, src, f32_ty, dst_temp);
        self.load_operand_byte_to_w(function_symbol, Operand::Temp(dst_temp), f32_ty, 3);
        self.program.push(AsmLine::Instr(AsmInstr::Andlw(0x7F)));
        self.store_w_to_temp_byte(function_symbol, dst_temp, 3);
    }

    /// Lowers one Phase 17 function-pointer call through a generated dispatch-ID trampoline.
    fn emit_indirect_call(
        &mut self,
        function: &IrFunction,
        callee: Operand,
        signature: Type,
        args: &[Operand],
        dst: Option<usize>,
        diagnostics: &mut DiagnosticBag,
    ) {
        if function.is_interrupt {
            diagnostics.error(
                "backend",
                None,
                format!(
                    "interrupt handler `{}` reached function-pointer call lowering",
                    self.symbol_name(function.symbol)
                ),
                Some("phase 17 forbids function-pointer calls inside ISRs".to_string()),
            );
            if let Some(dst) = dst {
                self.clear_temp(function.symbol, dst, function.temp_types[dst]);
            }
            return;
        }

        let Some(group_ty) = self
            .function_pointer_dispatch_group(signature)
            .map(|group| group.ty)
        else {
            diagnostics.error(
                "backend",
                None,
                format!(
                    "missing function-pointer dispatcher group for signature `{}`",
                    signature
                ),
                None,
            );
            if let Some(dst) = dst {
                self.clear_temp(function.symbol, dst, function.temp_types[dst]);
            }
            return;
        };

        let parameter_types = self.function_pointer_param_types(signature);
        let arg_bytes = self.function_pointer_arg_bytes(signature);
        self.emit_stack_growth_check(arg_bytes, &format!("indirect call {} arguments", signature));
        for (index, arg) in args.iter().enumerate() {
            let Some(param_ty) = parameter_types.get(index).copied() else {
                diagnostics.error(
                    "backend",
                    None,
                    "indirect call passes more arguments than its signature permits",
                    None,
                );
                continue;
            };
            self.push_operand(function.symbol, *arg, param_ty);
        }

        self.load_operand_byte_to_w(function.symbol, callee, signature, 0);
        self.store_w_to_addr(self.layout.helpers.scratch0);
        self.load_operand_byte_to_w(function.symbol, callee, signature, 1);
        self.store_w_to_addr(self.layout.helpers.scratch1);

        let label = function_pointer_dispatch_label(group_ty);
        self.program
            .push(AsmLine::Instr(AsmInstr::SetPage(label.clone())));
        self.program.push(AsmLine::Instr(AsmInstr::Call(label)));
        self.restore_code_page_after_call();

        if dst.is_some() {
            self.store_w_to_addr(self.layout.helpers.w_save);
        }
        if arg_bytes != 0 {
            self.add_immediate_to_pair(self.layout.helpers.stack_ptr, negate_u16(arg_bytes));
        }

        if let Some(dst) = dst {
            let dst_ty = function.temp_types[dst];
            self.load_addr_to_w(self.layout.helpers.w_save);
            self.store_w_to_temp_byte(function.symbol, dst, 0);
            for byte in 1..dst_ty.byte_width() {
                self.load_return_byte_to_w(byte);
                self.store_w_to_temp_byte(function.symbol, dst, byte);
            }
        }
    }

    /// Lowers one Phase 14 ROM-byte read using either an inline constant or RETLW-table dispatch.
    fn emit_rom_read8(
        &mut self,
        function: &IrFunction,
        symbol: SymbolId,
        index: Operand,
        dst: usize,
        diagnostics: &mut DiagnosticBag,
    ) {
        let index_ty = self.operand_type(function, index);
        let Some(bytes) = self.rom_object_bytes(symbol) else {
            diagnostics.error(
                "backend",
                None,
                format!(
                    "ROM object `{}` needs a byte-array initializer in phase 14",
                    self.symbol_name(symbol)
                ),
                None,
            );
            self.clear_temp(function.symbol, dst, function.temp_types[dst]);
            return;
        };
        let len = bytes.len();
        if len == 0 || len > 255 {
            diagnostics.error(
                "backend",
                None,
                format!(
                    "ROM object `{}` has unsupported phase 14 byte-table length {}",
                    self.symbol_name(symbol),
                    len
                ),
                None,
            );
            self.clear_temp(function.symbol, dst, function.temp_types[dst]);
            return;
        }
        if let Some(index_value) = constant_operand_value(index, index_ty)
            .map(|value| normalize_value(value, index_ty) as usize)
        {
            let byte = bytes.get(index_value).copied().unwrap_or(0);
            self.emit_const_to_w(byte);
            self.store_w_to_temp_byte(function.symbol, dst, 0);
            return;
        }
        if function.is_interrupt {
            diagnostics.error(
                "backend",
                None,
                format!(
                    "interrupt handler `{}` reached dynamic ROM-byte lowering in phase 14",
                    self.symbol_name(function.symbol)
                ),
                Some("only constant-index ROM reads are allowed inside ISRs".to_string()),
            );
            self.clear_temp(function.symbol, dst, function.temp_types[dst]);
            return;
        }

        let read_label = self.unique_label("rom_read");
        let miss_label = self.unique_label("rom_miss");
        let done_label = self.unique_label("rom_done");
        self.emit_unsigned_relation_branch(
            function.symbol,
            index,
            Operand::Constant(len as i64),
            index_ty,
            BinaryOp::Less,
            BranchTargets {
                then_label: &read_label,
                else_label: &miss_label,
            },
        );

        self.program.push(AsmLine::Label(read_label.clone()));
        self.emit_dynamic_rom_byte_call(function.symbol, symbol, index, index_ty);
        self.store_w_to_temp_byte(function.symbol, dst, 0);
        self.jump_to_label(&done_label);

        self.program.push(AsmLine::Label(miss_label));
        self.emit_const_to_w(0);
        self.store_w_to_temp_byte(function.symbol, dst, 0);
        self.program.push(AsmLine::Label(done_label));
    }

    /// Lowers one Phase 14 ROM-word read using little-endian byte packing in one RETLW table.
    fn emit_rom_read16(
        &mut self,
        function: &IrFunction,
        symbol: SymbolId,
        index: Operand,
        dst: usize,
        diagnostics: &mut DiagnosticBag,
    ) {
        let index_ty = self.operand_type(function, index);
        let Some(bytes) = self.rom_object_bytes(symbol) else {
            diagnostics.error(
                "backend",
                None,
                format!(
                    "ROM object `{}` needs a byte-array initializer in phase 14",
                    self.symbol_name(symbol)
                ),
                None,
            );
            self.clear_temp(function.symbol, dst, function.temp_types[dst]);
            return;
        };
        if bytes.len() % 2 != 0 {
            diagnostics.error(
                "backend",
                None,
                format!(
                    "ROM object `{}` has an invalid 16-bit byte layout in phase 14",
                    self.symbol_name(symbol)
                ),
                None,
            );
            self.clear_temp(function.symbol, dst, function.temp_types[dst]);
            return;
        }

        let len = bytes.len() / 2;
        if len == 0 || bytes.len() > 255 {
            diagnostics.error(
                "backend",
                None,
                format!(
                    "ROM object `{}` has unsupported phase 14 word-table length {} ({} bytes)",
                    self.symbol_name(symbol),
                    len,
                    bytes.len()
                ),
                Some(
                    "keep each 16-bit ROM table within one 255-byte RETLW payload page".to_string(),
                ),
            );
            self.clear_temp(function.symbol, dst, function.temp_types[dst]);
            return;
        }
        if let Some(index_value) = constant_operand_value(index, index_ty)
            .map(|value| normalize_value(value, index_ty) as usize)
        {
            let byte_index = index_value.saturating_mul(2);
            let lo = bytes.get(byte_index).copied().unwrap_or(0);
            let hi = bytes.get(byte_index + 1).copied().unwrap_or(0);
            self.emit_const_to_w(lo);
            self.store_w_to_temp_byte(function.symbol, dst, 0);
            self.emit_const_to_w(hi);
            self.store_w_to_temp_byte(function.symbol, dst, 1);
            return;
        }
        if function.is_interrupt {
            diagnostics.error(
                "backend",
                None,
                format!(
                    "interrupt handler `{}` reached dynamic ROM-word lowering in phase 14",
                    self.symbol_name(function.symbol)
                ),
                Some("only constant-index ROM reads are allowed inside ISRs".to_string()),
            );
            self.clear_temp(function.symbol, dst, function.temp_types[dst]);
            return;
        }

        let read_label = self.unique_label("rom16_read");
        let miss_label = self.unique_label("rom16_miss");
        let done_label = self.unique_label("rom16_done");
        self.emit_unsigned_relation_branch(
            function.symbol,
            index,
            Operand::Constant(len as i64),
            index_ty,
            BinaryOp::Less,
            BranchTargets {
                then_label: &read_label,
                else_label: &miss_label,
            },
        );

        self.program.push(AsmLine::Label(read_label.clone()));
        self.load_operand_byte_to_w(function.symbol, index, index_ty, 0);
        self.store_w_to_addr(self.layout.helpers.w_save);
        self.select_bank(self.layout.helpers.w_save);
        self.program.push(AsmLine::Instr(AsmInstr::Addwf {
            f: low7(self.layout.helpers.w_save),
            d: Dest::W,
        }));
        self.store_w_to_addr(self.layout.helpers.w_save);
        self.load_addr_to_w(self.layout.helpers.w_save);
        self.emit_dynamic_rom_byte_call_from_w(symbol);
        self.store_w_to_temp_byte(function.symbol, dst, 0);

        self.load_addr_to_w(self.layout.helpers.w_save);
        self.program.push(AsmLine::Instr(AsmInstr::Addlw(1)));
        self.store_w_to_addr(self.layout.helpers.w_save);
        self.load_addr_to_w(self.layout.helpers.w_save);
        self.emit_dynamic_rom_byte_call_from_w(symbol);
        self.store_w_to_temp_byte(function.symbol, dst, 1);
        self.jump_to_label(&done_label);

        self.program.push(AsmLine::Label(miss_label));
        self.emit_const_to_w(0);
        self.store_w_to_temp_byte(function.symbol, dst, 0);
        self.emit_const_to_w(0);
        self.store_w_to_temp_byte(function.symbol, dst, 1);
        self.program.push(AsmLine::Label(done_label));
    }

    /// Lowers one fixed-point or float ROM double-word read from little-endian bytes.
    fn emit_rom_read32(
        &mut self,
        function: &IrFunction,
        symbol: SymbolId,
        index: Operand,
        dst: usize,
        diagnostics: &mut DiagnosticBag,
    ) {
        let index_ty = self.operand_type(function, index);
        let Some(bytes) = self.rom_object_bytes(symbol) else {
            diagnostics.error(
                "backend",
                None,
                format!(
                    "ROM object `{}` needs a byte-array initializer for a 32-bit ROM read",
                    self.symbol_name(symbol)
                ),
                None,
            );
            self.clear_temp(function.symbol, dst, function.temp_types[dst]);
            return;
        };
        if bytes.len() % 4 != 0 {
            diagnostics.error(
                "backend",
                None,
                format!(
                    "ROM object `{}` has an invalid 32-bit ROM byte layout",
                    self.symbol_name(symbol)
                ),
                None,
            );
            self.clear_temp(function.symbol, dst, function.temp_types[dst]);
            return;
        }

        let len = bytes.len() / 4;
        if len == 0 || bytes.len() > 255 {
            diagnostics.error(
                "backend",
                None,
                format!(
                    "ROM object `{}` has unsupported 32-bit ROM table length {} ({} bytes)",
                    self.symbol_name(symbol),
                    len,
                    bytes.len()
                ),
                Some(
                    "keep each 32-bit ROM table within one 255-byte RETLW payload page".to_string(),
                ),
            );
            self.clear_temp(function.symbol, dst, function.temp_types[dst]);
            return;
        }
        if let Some(index_value) = constant_operand_value(index, index_ty)
            .map(|value| normalize_value(value, index_ty) as usize)
        {
            let byte_index = index_value.saturating_mul(4);
            let value = [
                bytes.get(byte_index).copied().unwrap_or(0),
                bytes.get(byte_index + 1).copied().unwrap_or(0),
                bytes.get(byte_index + 2).copied().unwrap_or(0),
                bytes.get(byte_index + 3).copied().unwrap_or(0),
            ];
            for (byte, value) in value.into_iter().enumerate() {
                self.emit_const_to_w(value);
                self.store_w_to_temp_byte(function.symbol, dst, byte);
            }
            return;
        }
        if function.is_interrupt {
            diagnostics.error(
                "backend",
                None,
                format!(
                    "interrupt handler `{}` reached dynamic ROM double-word lowering",
                    self.symbol_name(function.symbol)
                ),
                Some("only constant-index ROM reads are allowed inside ISRs".to_string()),
            );
            self.clear_temp(function.symbol, dst, function.temp_types[dst]);
            return;
        }

        let read_label = self.unique_label("rom32_read");
        let miss_label = self.unique_label("rom32_miss");
        let done_label = self.unique_label("rom32_done");
        self.emit_unsigned_relation_branch(
            function.symbol,
            index,
            Operand::Constant(len as i64),
            index_ty,
            BinaryOp::Less,
            BranchTargets {
                then_label: &read_label,
                else_label: &miss_label,
            },
        );

        self.program.push(AsmLine::Label(read_label.clone()));
        self.load_operand_byte_to_w(function.symbol, index, index_ty, 0);
        self.store_w_to_addr(self.layout.helpers.w_save);
        for _ in 0..2 {
            self.select_bank(self.layout.helpers.w_save);
            self.program.push(AsmLine::Instr(AsmInstr::Addwf {
                f: low7(self.layout.helpers.w_save),
                d: Dest::W,
            }));
            self.store_w_to_addr(self.layout.helpers.w_save);
            self.load_addr_to_w(self.layout.helpers.w_save);
        }
        for byte in 0..4 {
            if byte != 0 {
                self.load_addr_to_w(self.layout.helpers.w_save);
                self.program.push(AsmLine::Instr(AsmInstr::Addlw(1)));
                self.store_w_to_addr(self.layout.helpers.w_save);
            }
            self.load_addr_to_w(self.layout.helpers.w_save);
            self.emit_dynamic_rom_byte_call_from_w(symbol);
            self.store_w_to_temp_byte(function.symbol, dst, byte);
        }
        self.jump_to_label(&done_label);

        self.program.push(AsmLine::Label(miss_label));
        for byte in 0..4 {
            self.emit_const_to_w(0);
            self.store_w_to_temp_byte(function.symbol, dst, byte);
        }
        self.program.push(AsmLine::Label(done_label));
    }

    /// Emits one dynamic ROM-byte table call after first loading the byte index into `W`.
    fn emit_dynamic_rom_byte_call(
        &mut self,
        function_symbol: SymbolId,
        symbol: SymbolId,
        index: Operand,
        index_ty: Type,
    ) {
        self.load_operand_byte_to_w(function_symbol, index, index_ty, 0);
        self.emit_dynamic_rom_byte_call_from_w(symbol);
    }

    /// Emits one ROM RETLW-table call assuming the byte index already lives in `W`.
    fn emit_dynamic_rom_byte_call_from_w(&mut self, symbol: SymbolId) {
        let label = rom_object_label(symbol);
        self.program
            .push(AsmLine::Instr(AsmInstr::SetPclPage(label.clone())));
        self.program.push(AsmLine::Instr(AsmInstr::Call(label)));
        self.restore_code_page_after_call();
    }

    /// Returns the flattened ROM byte payload for one declared program-memory object.
    fn rom_object_bytes(&self, symbol: SymbolId) -> Option<&[u8]> {
        self.typed_program
            .globals
            .iter()
            .find(|global| global.symbol == symbol)
            .and_then(|global| match &global.initializer {
                Some(TypedGlobalInitializer::Bytes(bytes)) => Some(bytes.as_slice()),
                Some(TypedGlobalInitializer::Scalar(_))
                | Some(TypedGlobalInitializer::Address { .. })
                | None => None,
            })
    }

    /// Places a return operand into the Phase 4 return convention locations.
    fn emit_return_value(&mut self, function_symbol: SymbolId, value: Operand, return_ty: Type) {
        for byte in 1..return_ty.byte_width() {
            self.load_operand_byte_to_w(function_symbol, value, return_ty, byte);
            self.store_w_to_return_byte(byte);
        }
        self.load_operand_byte_to_w(function_symbol, value, return_ty, 0);
    }

    /// Returns the ABI helper slot used for a non-W return byte.
    fn return_slot(&self, byte_index: usize) -> u16 {
        match byte_index {
            1 => self.layout.helpers.return_high,
            2 => self.layout.helpers.return_upper0,
            3 => self.layout.helpers.return_upper1,
            _ => unreachable!("unsupported ABI return byte"),
        }
    }

    /// Stores `W` into the ABI return slot for byte 1..3.
    fn store_w_to_return_byte(&mut self, byte_index: usize) {
        self.store_w_to_addr(self.return_slot(byte_index));
    }

    /// Loads byte 1..3 from the ABI return slots into `W`.
    fn load_return_byte_to_w(&mut self, byte_index: usize) {
        self.load_addr_to_w(self.return_slot(byte_index));
    }

    /// Lowers a typed IR branch condition into PIC16 compare-and-branch sequences.
    fn emit_branch(
        &mut self,
        function: &IrFunction,
        condition: &IrCondition,
        then_label: &str,
        else_label: &str,
        diagnostics: &mut DiagnosticBag,
    ) {
        match condition {
            IrCondition::NonZero { value, ty } => {
                self.emit_nonzero_branch(function.symbol, *value, *ty, then_label, else_label);
            }
            IrCondition::Compare { op, lhs, rhs, ty } => {
                let targets = BranchTargets {
                    then_label,
                    else_label,
                };
                if let (Operand::Constant(lhs), Operand::Constant(rhs)) = (lhs, rhs) {
                    if compare_rel(*op, *lhs, *rhs, *ty) {
                        self.jump_to_label(targets.then_label);
                    } else {
                        self.jump_to_label(targets.else_label);
                    }
                    return;
                }

                if ty.is_float() {
                    self.emit_float_compare_branch(function, *op, *lhs, *rhs, targets, diagnostics);
                    return;
                }

                match op {
                    BinaryOp::Equal => {
                        self.emit_equality_branch(function.symbol, *lhs, *rhs, *ty, false, targets)
                    }
                    BinaryOp::NotEqual => {
                        self.emit_equality_branch(function.symbol, *lhs, *rhs, *ty, true, targets)
                    }
                    BinaryOp::Less
                    | BinaryOp::LessEqual
                    | BinaryOp::Greater
                    | BinaryOp::GreaterEqual => {
                        if ty.is_signed() {
                            self.emit_signed_relation_branch(
                                function.symbol,
                                *lhs,
                                *rhs,
                                *ty,
                                *op,
                                targets,
                            );
                        } else {
                            self.emit_unsigned_relation_branch(
                                function.symbol,
                                *lhs,
                                *rhs,
                                *ty,
                                *op,
                                targets,
                            );
                        }
                    }
                    BinaryOp::Add
                    | BinaryOp::Sub
                    | BinaryOp::Multiply
                    | BinaryOp::Divide
                    | BinaryOp::Modulo
                    | BinaryOp::ShiftLeft
                    | BinaryOp::ShiftRight
                    | BinaryOp::BitAnd
                    | BinaryOp::BitOr
                    | BinaryOp::BitXor
                    | BinaryOp::LogicalAnd
                    | BinaryOp::LogicalOr => {
                        diagnostics.error(
                            "backend",
                            None,
                            format!("non-comparison `{op:?}` reached compare branch lowering"),
                            None,
                        );
                        self.jump_to_label(targets.else_label);
                    }
                }
            }
        }
    }

    /// Emits a branch driven by the shared finite f32 compare helper.
    fn emit_float_compare_branch(
        &mut self,
        function: &IrFunction,
        op: BinaryOp,
        lhs: Operand,
        rhs: Operand,
        targets: BranchTargets<'_>,
        diagnostics: &mut DiagnosticBag,
    ) {
        if function.is_interrupt {
            diagnostics.error(
                "backend",
                None,
                format!(
                    "interrupt handler `{}` reached runtime-helper lowering for float comparison",
                    self.symbol_name(function.symbol)
                ),
                Some("phase 29 forbids float helper calls inside ISRs".to_string()),
            );
            self.jump_to_label(targets.else_label);
            return;
        }

        let helper = RuntimeHelper::F32Cmp;
        let info = helper.info();
        self.used_helpers.insert(helper);
        self.emit_stack_growth_check(
            info.arg_bytes,
            &format!("runtime helper {} arguments", info.label),
        );
        self.push_operand(function.symbol, lhs, info.operand_ty);
        self.push_operand(function.symbol, rhs, info.operand_ty);
        self.program
            .push(AsmLine::Instr(AsmInstr::SetPage(info.label.to_string())));
        self.program
            .push(AsmLine::Instr(AsmInstr::Call(info.label.to_string())));
        self.restore_code_page_after_call();
        self.store_w_to_addr(self.layout.helpers.scratch0);
        self.add_immediate_to_pair(self.layout.helpers.stack_ptr, negate_u16(info.arg_bytes));

        match op {
            BinaryOp::Equal => {
                self.emit_scratch0_equals_branch(0, targets.then_label, targets.else_label)
            }
            BinaryOp::NotEqual => {
                self.emit_scratch0_nonzero_branch(targets.then_label, targets.else_label)
            }
            BinaryOp::Less => {
                self.emit_scratch0_equals_branch(0xFF, targets.then_label, targets.else_label)
            }
            BinaryOp::LessEqual => {
                self.emit_scratch0_equals_branch(1, targets.else_label, targets.then_label)
            }
            BinaryOp::Greater => {
                self.emit_scratch0_equals_branch(1, targets.then_label, targets.else_label)
            }
            BinaryOp::GreaterEqual => {
                self.emit_scratch0_equals_branch(0xFF, targets.else_label, targets.then_label)
            }
            _ => {
                diagnostics.error(
                    "backend",
                    None,
                    format!("non-comparison `{op:?}` reached float compare lowering"),
                    None,
                );
                self.jump_to_label(targets.else_label);
            }
        }
    }

    fn emit_scratch0_equals_branch(&mut self, value: u8, then_label: &str, else_label: &str) {
        self.load_addr_to_w(self.layout.helpers.scratch0);
        self.program.push(AsmLine::Instr(AsmInstr::Xorlw(value)));
        self.branch_if_bit_set(low7(STATUS_ADDR), STATUS_Z_BIT, then_label);
        self.jump_to_label(else_label);
    }

    fn emit_scratch0_nonzero_branch(&mut self, then_label: &str, else_label: &str) {
        self.load_addr_to_w(self.layout.helpers.scratch0);
        self.branch_if_bit_clear(low7(STATUS_ADDR), STATUS_Z_BIT, then_label);
        self.jump_to_label(else_label);
    }

    /// Branches on whether an 8-bit or 16-bit operand is zero or non-zero.
    fn emit_nonzero_branch(
        &mut self,
        function_symbol: SymbolId,
        value: Operand,
        ty: Type,
        then_label: &str,
        else_label: &str,
    ) {
        if let Operand::Constant(value) = value {
            if eval_binary(BinaryOp::NotEqual, value, 0, ty, Type::new(ScalarType::U8)) != 0 {
                self.jump_to_label(then_label);
            } else {
                self.jump_to_label(else_label);
            }
            return;
        }

        for byte in 0..ty.byte_width() {
            self.load_operand_byte_to_w(function_symbol, value, ty, byte);
            self.branch_if_bit_clear(low7(STATUS_ADDR), STATUS_Z_BIT, then_label);
        }
        self.jump_to_label(else_label);
    }

    /// Emits equality or inequality branching, handling 16-bit values byte by byte.
    fn emit_equality_branch(
        &mut self,
        function_symbol: SymbolId,
        lhs: Operand,
        rhs: Operand,
        ty: Type,
        invert: bool,
        targets: BranchTargets<'_>,
    ) {
        let mismatch_label = if invert {
            targets.then_label
        } else {
            targets.else_label
        };
        let equal_label = if invert {
            targets.else_label
        } else {
            targets.then_label
        };
        for byte in (0..ty.byte_width()).rev() {
            self.compare_byte(function_symbol, lhs, rhs, ty, byte);
            self.branch_if_bit_clear(low7(STATUS_ADDR), STATUS_Z_BIT, mismatch_label);
        }
        self.jump_to_label(equal_label);
    }

    /// Emits unsigned relational branching using PIC16 carry and zero flags.
    fn emit_unsigned_relation_branch(
        &mut self,
        function_symbol: SymbolId,
        lhs: Operand,
        rhs: Operand,
        ty: Type,
        op: BinaryOp,
        targets: BranchTargets<'_>,
    ) {
        for byte in (0..ty.byte_width()).rev() {
            self.compare_byte(function_symbol, lhs, rhs, ty, byte);
            if byte != 0 {
                let next_label = self.unique_label("cmp_next");
                self.branch_if_bit_set(low7(STATUS_ADDR), STATUS_Z_BIT, &next_label);
                self.branch_on_unsigned_result(op, targets.then_label, targets.else_label);
                self.program.push(AsmLine::Label(next_label));
            } else {
                self.branch_on_unsigned_result(op, targets.then_label, targets.else_label);
            }
        }
    }

    /// Emits signed relational branching by splitting sign-mismatch and same-sign cases.
    fn emit_signed_relation_branch(
        &mut self,
        function_symbol: SymbolId,
        lhs: Operand,
        rhs: Operand,
        ty: Type,
        op: BinaryOp,
        targets: BranchTargets<'_>,
    ) {
        let same_sign = self.unique_label("cmp_same_sign");
        let high_index = ty.byte_width() - 1;

        self.load_operand_byte_to_w(function_symbol, lhs, ty, high_index);
        self.store_w_to_addr(self.layout.helpers.scratch0);
        self.load_operand_byte_to_w(function_symbol, rhs, ty, high_index);
        self.store_w_to_addr(self.layout.helpers.scratch1);
        self.load_addr_to_w(self.layout.helpers.scratch0);
        self.select_bank(self.layout.helpers.scratch1);
        self.program.push(AsmLine::Instr(AsmInstr::Xorwf {
            f: low7(self.layout.helpers.scratch1),
            d: Dest::F,
        }));
        self.branch_if_bit_clear(low7(self.layout.helpers.scratch1), 7, &same_sign);

        match op {
            BinaryOp::Less | BinaryOp::LessEqual => {
                self.branch_if_bit_set(low7(self.layout.helpers.scratch0), 7, targets.then_label);
                self.jump_to_label(targets.else_label);
            }
            BinaryOp::Greater | BinaryOp::GreaterEqual => {
                self.branch_if_bit_set(low7(self.layout.helpers.scratch0), 7, targets.else_label);
                self.jump_to_label(targets.then_label);
            }
            _ => unreachable!("signed relation op"),
        }

        self.program.push(AsmLine::Label(same_sign));
        self.emit_unsigned_relation_branch(function_symbol, lhs, rhs, ty, op, targets);
    }

    /// Interprets carry/zero flags after subtraction for unsigned relation operators.
    fn branch_on_unsigned_result(&mut self, op: BinaryOp, then_label: &str, else_label: &str) {
        match op {
            BinaryOp::Less => {
                self.branch_if_bit_clear(low7(STATUS_ADDR), STATUS_C_BIT, then_label);
                self.jump_to_label(else_label);
            }
            BinaryOp::LessEqual => {
                self.branch_if_bit_clear(low7(STATUS_ADDR), STATUS_C_BIT, then_label);
                self.branch_if_bit_set(low7(STATUS_ADDR), STATUS_Z_BIT, then_label);
                self.jump_to_label(else_label);
            }
            BinaryOp::Greater => {
                self.branch_if_bit_clear(low7(STATUS_ADDR), STATUS_C_BIT, else_label);
                self.branch_if_bit_set(low7(STATUS_ADDR), STATUS_Z_BIT, else_label);
                self.jump_to_label(then_label);
            }
            BinaryOp::GreaterEqual => {
                self.branch_if_bit_set(low7(STATUS_ADDR), STATUS_C_BIT, then_label);
                self.jump_to_label(else_label);
            }
            _ => unreachable!("unsigned relation op"),
        }
    }

    /// Subtracts one operand byte from another and leaves the compare result in flags.
    fn compare_byte(
        &mut self,
        function_symbol: SymbolId,
        lhs: Operand,
        rhs: Operand,
        ty: Type,
        byte_index: usize,
    ) {
        self.load_operand_byte_to_w(function_symbol, lhs, ty, byte_index);
        self.store_w_to_addr(self.layout.helpers.scratch0);
        self.load_operand_byte_to_w(function_symbol, rhs, ty, byte_index);
        self.select_bank(self.layout.helpers.scratch0);
        self.program.push(AsmLine::Instr(AsmInstr::Subwf {
            f: low7(self.layout.helpers.scratch0),
            d: Dest::W,
        }));
    }

    /// Emits addition with explicit carry propagation across every byte.
    fn emit_add(
        &mut self,
        function_symbol: SymbolId,
        lhs: Operand,
        rhs: Operand,
        ty: Type,
        dst_temp: usize,
    ) {
        for byte in 0..ty.byte_width() {
            self.load_operand_byte_to_w(function_symbol, lhs, ty, byte);
            self.store_w_to_addr(self.layout.helpers.scratch0);
            self.load_operand_byte_to_w(function_symbol, rhs, ty, byte);
            if byte != 0 {
                self.clear_addr(self.layout.helpers.scratch1);
                self.program.push(AsmLine::Instr(AsmInstr::Btfsc {
                    f: low7(STATUS_ADDR),
                    b: STATUS_C_BIT,
                }));
                self.program.push(AsmLine::Instr(AsmInstr::Addlw(1)));
                self.program.push(AsmLine::Instr(AsmInstr::Btfsc {
                    f: low7(STATUS_ADDR),
                    b: STATUS_C_BIT,
                }));
                self.program.push(AsmLine::Instr(AsmInstr::Bsf {
                    f: low7(self.layout.helpers.scratch1),
                    b: 0,
                }));
            }
            self.store_w_to_addr(self.layout.helpers.w_save);
            self.load_addr_to_w(self.layout.helpers.scratch0);
            self.select_bank(self.layout.helpers.w_save);
            self.program.push(AsmLine::Instr(AsmInstr::Addwf {
                f: low7(self.layout.helpers.w_save),
                d: Dest::W,
            }));
            if byte != 0 {
                self.select_bank(self.layout.helpers.scratch1);
                self.restore_code_page_after_call();
                self.program.push(AsmLine::Instr(AsmInstr::Btfsc {
                    f: low7(self.layout.helpers.scratch1),
                    b: 0,
                }));
                self.program.push(AsmLine::Instr(AsmInstr::Bsf {
                    f: low7(STATUS_ADDR),
                    b: STATUS_C_BIT,
                }));
            }
            self.store_w_to_temp_byte(function_symbol, dst_temp, byte);
        }
    }

    /// Emits subtraction with explicit borrow propagation across every byte.
    fn emit_sub(
        &mut self,
        function_symbol: SymbolId,
        lhs: Operand,
        rhs: Operand,
        ty: Type,
        dst_temp: usize,
    ) {
        for byte in 0..ty.byte_width() {
            self.load_operand_byte_to_w(function_symbol, lhs, ty, byte);
            self.store_w_to_addr(self.layout.helpers.scratch0);
            self.load_operand_byte_to_w(function_symbol, rhs, ty, byte);
            if byte != 0 {
                let no_borrow = self.unique_label("sub_no_borrow");
                self.clear_addr(self.layout.helpers.scratch1);
                self.branch_if_bit_set(low7(STATUS_ADDR), STATUS_C_BIT, &no_borrow);
                self.program.push(AsmLine::Instr(AsmInstr::Addlw(1)));
                self.program.push(AsmLine::Instr(AsmInstr::Btfsc {
                    f: low7(STATUS_ADDR),
                    b: STATUS_C_BIT,
                }));
                self.program.push(AsmLine::Instr(AsmInstr::Bsf {
                    f: low7(self.layout.helpers.scratch1),
                    b: 0,
                }));
                self.program.push(AsmLine::Label(no_borrow));
            }
            self.store_w_to_addr(self.layout.helpers.w_save);
            self.load_addr_to_w(self.layout.helpers.w_save);
            self.select_bank(self.layout.helpers.scratch0);
            self.program.push(AsmLine::Instr(AsmInstr::Subwf {
                f: low7(self.layout.helpers.scratch0),
                d: Dest::W,
            }));
            if byte != 0 {
                self.select_bank(self.layout.helpers.scratch1);
                self.restore_code_page_after_call();
                self.program.push(AsmLine::Instr(AsmInstr::Btfsc {
                    f: low7(self.layout.helpers.scratch1),
                    b: 0,
                }));
                self.program.push(AsmLine::Instr(AsmInstr::Bcf {
                    f: low7(STATUS_ADDR),
                    b: STATUS_C_BIT,
                }));
            }
            self.store_w_to_temp_byte(function_symbol, dst_temp, byte);
        }
    }

    /// Emits two's-complement negation for the requested integer width.
    fn emit_negate(&mut self, function_symbol: SymbolId, src: Operand, ty: Type, dst_temp: usize) {
        if ty.is_float() {
            for byte in 0..ty.byte_width() {
                self.load_operand_byte_to_w(function_symbol, src, ty, byte);
                if byte == 3 {
                    self.program.push(AsmLine::Instr(AsmInstr::Xorlw(0x80)));
                }
                self.store_w_to_temp_byte(function_symbol, dst_temp, byte);
            }
            return;
        }
        for byte in 0..ty.byte_width() {
            self.clear_addr(self.layout.helpers.scratch0);
            self.load_operand_byte_to_w(function_symbol, src, ty, byte);
            if byte != 0 {
                let no_borrow = self.unique_label("neg_no_borrow");
                self.clear_addr(self.layout.helpers.scratch1);
                self.branch_if_bit_set(low7(STATUS_ADDR), STATUS_C_BIT, &no_borrow);
                self.program.push(AsmLine::Instr(AsmInstr::Addlw(1)));
                self.program.push(AsmLine::Instr(AsmInstr::Btfsc {
                    f: low7(STATUS_ADDR),
                    b: STATUS_C_BIT,
                }));
                self.program.push(AsmLine::Instr(AsmInstr::Bsf {
                    f: low7(self.layout.helpers.scratch1),
                    b: 0,
                }));
                self.program.push(AsmLine::Label(no_borrow));
            }
            self.store_w_to_addr(self.layout.helpers.w_save);
            self.load_addr_to_w(self.layout.helpers.w_save);
            self.select_bank(self.layout.helpers.scratch0);
            self.program.push(AsmLine::Instr(AsmInstr::Subwf {
                f: low7(self.layout.helpers.scratch0),
                d: Dest::W,
            }));
            if byte != 0 {
                self.select_bank(self.layout.helpers.scratch1);
                self.restore_code_page_after_call();
                self.program.push(AsmLine::Instr(AsmInstr::Btfsc {
                    f: low7(self.layout.helpers.scratch1),
                    b: 0,
                }));
                self.program.push(AsmLine::Instr(AsmInstr::Bcf {
                    f: low7(STATUS_ADDR),
                    b: STATUS_C_BIT,
                }));
            }
            self.store_w_to_temp_byte(function_symbol, dst_temp, byte);
        }
    }

    /// Lowers Phase 5 arithmetic through inline fast paths or runtime helper calls.
    #[allow(clippy::too_many_arguments)]
    fn emit_phase_five_binary(
        &mut self,
        function: &IrFunction,
        op: BinaryOp,
        lhs: Operand,
        rhs: Operand,
        ty: Type,
        dst_temp: usize,
        diagnostics: &mut DiagnosticBag,
    ) {
        let function_symbol = function.symbol;
        let lhs_const = constant_operand_value(lhs, ty);
        let rhs_const = constant_operand_value(rhs, ty);

        if ty.is_float() {
            match op {
                BinaryOp::Multiply => {
                    if lhs_const == Some(0) || rhs_const == Some(0) {
                        self.clear_temp(function_symbol, dst_temp, ty);
                        self.helper_calls_avoided += 1;
                        return;
                    }
                    if lhs_const == Some(0x3F80_0000) {
                        self.copy_operand_to_temp(function_symbol, rhs, ty, dst_temp);
                        self.helper_calls_avoided += 1;
                        return;
                    }
                    if rhs_const == Some(0x3F80_0000) {
                        self.copy_operand_to_temp(function_symbol, lhs, ty, dst_temp);
                        self.helper_calls_avoided += 1;
                        return;
                    }
                    if lhs_const == Some(0x4000_0000) {
                        self.emit_float_scale_by_power_of_two(function_symbol, rhs, dst_temp, 1);
                        self.helper_calls_avoided += 1;
                        return;
                    }
                    if rhs_const == Some(0x4000_0000) {
                        self.emit_float_scale_by_power_of_two(function_symbol, lhs, dst_temp, 1);
                        self.helper_calls_avoided += 1;
                        return;
                    }
                }
                BinaryOp::Divide => {
                    if lhs_const == Some(0) {
                        self.clear_temp(function_symbol, dst_temp, ty);
                        self.helper_calls_avoided += 1;
                        return;
                    }
                    if rhs_const == Some(0x3F80_0000) {
                        self.copy_operand_to_temp(function_symbol, lhs, ty, dst_temp);
                        self.helper_calls_avoided += 1;
                        return;
                    }
                    if rhs_const == Some(0x4000_0000) {
                        self.emit_float_scale_by_power_of_two(function_symbol, lhs, dst_temp, -1);
                        self.helper_calls_avoided += 1;
                        return;
                    }
                }
                _ => {}
            }
            self.emit_runtime_binary_call(function, op, lhs, rhs, ty, dst_temp, diagnostics);
            return;
        }

        if ty.is_fixed() {
            self.emit_runtime_binary_call(function, op, lhs, rhs, ty, dst_temp, diagnostics);
            return;
        }

        match op {
            BinaryOp::Multiply => {
                if lhs_const == Some(0) || rhs_const == Some(0) {
                    self.clear_temp(function_symbol, dst_temp, ty);
                    self.helper_calls_avoided += 1;
                    return;
                }
                if lhs_const == Some(1) {
                    self.copy_operand_to_temp(function_symbol, rhs, ty, dst_temp);
                    self.helper_calls_avoided += 1;
                    return;
                }
                if rhs_const == Some(1) {
                    self.copy_operand_to_temp(function_symbol, lhs, ty, dst_temp);
                    self.helper_calls_avoided += 1;
                    return;
                }
                if let Some(shift) = normalized_power_of_two_shift(lhs_const, ty) {
                    self.emit_constant_shift(function_symbol, rhs, ty, dst_temp, shift, false);
                    self.helper_calls_avoided += 1;
                    return;
                }
                if let Some(shift) = normalized_power_of_two_shift(rhs_const, ty) {
                    self.emit_constant_shift(function_symbol, lhs, ty, dst_temp, shift, false);
                    self.helper_calls_avoided += 1;
                    return;
                }
            }
            BinaryOp::Divide => {
                if lhs_const == Some(0) {
                    self.clear_temp(function_symbol, dst_temp, ty);
                    self.helper_calls_avoided += 1;
                    return;
                }
                if rhs_const == Some(1) {
                    self.copy_operand_to_temp(function_symbol, lhs, ty, dst_temp);
                    self.helper_calls_avoided += 1;
                    return;
                }
                if ty.is_unsigned()
                    && let Some(shift) = normalized_power_of_two_shift(rhs_const, ty)
                {
                    self.emit_constant_shift(function_symbol, lhs, ty, dst_temp, shift, true);
                    self.helper_calls_avoided += 1;
                    return;
                }
            }
            BinaryOp::Modulo => {
                if lhs_const == Some(0) || rhs_const == Some(1) {
                    self.clear_temp(function_symbol, dst_temp, ty);
                    self.helper_calls_avoided += 1;
                    return;
                }
                if ty.is_unsigned()
                    && let Some(mask) = normalized_power_of_two_mask(rhs_const, ty)
                {
                    self.emit_constant_mask(function_symbol, lhs, ty, dst_temp, mask);
                    self.helper_calls_avoided += 1;
                    return;
                }
            }
            BinaryOp::ShiftLeft | BinaryOp::ShiftRight => {
                if let Some(count) = rhs_const.map(|value| normalize_value(value, ty) as usize) {
                    if count == 0 {
                        self.copy_operand_to_temp(function_symbol, lhs, ty, dst_temp);
                        self.helper_calls_avoided += 1;
                        return;
                    }
                    self.emit_constant_shift(
                        function_symbol,
                        lhs,
                        ty,
                        dst_temp,
                        count,
                        op == BinaryOp::ShiftRight,
                    );
                    self.helper_calls_avoided += 1;
                    return;
                }
            }
            _ => unreachable!("phase five arithmetic op"),
        }

        self.emit_runtime_binary_call(function, op, lhs, rhs, ty, dst_temp, diagnostics);
    }

    /// Emits one constant-count shift directly in the caller frame without a helper call.
    fn emit_constant_shift(
        &mut self,
        function_symbol: SymbolId,
        src: Operand,
        ty: Type,
        dst_temp: usize,
        count: usize,
        shift_right: bool,
    ) {
        self.copy_operand_to_temp(function_symbol, src, ty, dst_temp);
        let offset = self.temp_offset(function_symbol, dst_temp);
        for _ in 0..count {
            if shift_right {
                self.shift_current_frame_value_right(offset, ty, ty.is_signed());
            } else {
                self.shift_current_frame_value_left(offset, ty);
            }
        }
    }

    /// Emits an unsigned power-of-two modulo as a constant mask instead of a helper call.
    fn emit_constant_mask(
        &mut self,
        function_symbol: SymbolId,
        src: Operand,
        ty: Type,
        dst_temp: usize,
        mask: i64,
    ) {
        for byte in 0..ty.byte_width() {
            self.load_operand_byte_to_w(function_symbol, src, ty, byte);
            let mask_byte = value_byte(mask, ty, byte);
            if mask_byte != 0xFF {
                self.program
                    .push(AsmLine::Instr(AsmInstr::Andlw(mask_byte)));
            }
            self.store_w_to_temp_byte(function_symbol, dst_temp, byte);
        }
    }

    /// Applies a narrow finite f32 exponent delta used for common `* 2.0f` and `/ 2.0f`.
    fn emit_float_scale_by_power_of_two(
        &mut self,
        function_symbol: SymbolId,
        src: Operand,
        dst_temp: usize,
        exponent_delta: i8,
    ) {
        let ty = Type::new(ScalarType::F32);
        self.copy_operand_to_temp(function_symbol, src, ty, dst_temp);
        let offset = self.temp_offset(function_symbol, dst_temp);
        let byte2 = offset + 2;
        let byte3 = offset + 3;
        let set_low_label = self.unique_label("f32_exp_set_low");
        let adjust_high_label = self.unique_label("f32_exp_adjust_high");
        let done_label = self.unique_label("f32_exp_done");

        self.prepare_pointer_from_pair(self.layout.helpers.frame_ptr, byte2);
        self.select_bank(INDF_ADDR);
        if exponent_delta > 0 {
            self.branch_if_bit_clear(low7(INDF_ADDR), 7, &set_low_label);
            self.program.push(AsmLine::Instr(AsmInstr::Bcf {
                f: low7(INDF_ADDR),
                b: 7,
            }));
            self.jump_to_label(&adjust_high_label);
            self.program.push(AsmLine::Label(set_low_label));
            self.program.push(AsmLine::Instr(AsmInstr::Bsf {
                f: low7(INDF_ADDR),
                b: 7,
            }));
            self.jump_to_label(&done_label);
            self.program.push(AsmLine::Label(adjust_high_label));
            self.load_frame_byte_to_w(function_symbol, byte3);
            self.program.push(AsmLine::Instr(AsmInstr::Addlw(1)));
            self.store_w_to_frame_byte(function_symbol, byte3);
        } else {
            self.branch_if_bit_set(low7(INDF_ADDR), 7, &adjust_high_label);
            self.program.push(AsmLine::Instr(AsmInstr::Bsf {
                f: low7(INDF_ADDR),
                b: 7,
            }));
            self.load_frame_byte_to_w(function_symbol, byte3);
            self.program.push(AsmLine::Instr(AsmInstr::Addlw(0xFF)));
            self.store_w_to_frame_byte(function_symbol, byte3);
            self.jump_to_label(&done_label);
            self.program.push(AsmLine::Label(adjust_high_label));
            self.program.push(AsmLine::Instr(AsmInstr::Bcf {
                f: low7(INDF_ADDR),
                b: 7,
            }));
        }
        self.program.push(AsmLine::Label(done_label));
    }

    /// Emits one helper call for a Phase 5 arithmetic operation under the stack-first ABI.
    #[allow(clippy::too_many_arguments)]
    fn emit_runtime_binary_call(
        &mut self,
        function: &IrFunction,
        op: BinaryOp,
        lhs: Operand,
        rhs: Operand,
        ty: Type,
        dst_temp: usize,
        diagnostics: &mut DiagnosticBag,
    ) {
        let function_symbol = function.symbol;
        if function.is_interrupt {
            diagnostics.error(
                "backend",
                None,
                format!(
                    "interrupt handler `{}` reached runtime-helper lowering for `{op:?}`",
                    self.symbol_name(function_symbol)
                ),
                Some("phase 6 forbids helper calls inside ISRs".to_string()),
            );
            self.clear_temp(function_symbol, dst_temp, ty);
            return;
        }
        let helper = binary_helper(op, ty).expect("phase five helper exists");
        let info = helper.info();
        self.used_helpers.insert(helper);
        self.emit_stack_growth_check(
            info.arg_bytes,
            &format!("runtime helper {} arguments", info.label),
        );
        self.push_operand(function_symbol, lhs, info.operand_ty);
        self.push_operand(function_symbol, rhs, info.operand_ty);
        self.program
            .push(AsmLine::Instr(AsmInstr::SetPage(info.label.to_string())));
        self.program
            .push(AsmLine::Instr(AsmInstr::Call(info.label.to_string())));
        self.restore_code_page_after_call();
        self.store_w_to_addr(self.layout.helpers.w_save);
        self.add_immediate_to_pair(self.layout.helpers.stack_ptr, negate_u16(info.arg_bytes));
        self.load_addr_to_w(self.layout.helpers.w_save);
        self.store_w_to_temp_byte(function_symbol, dst_temp, 0);
        for byte in 1..ty.byte_width() {
            self.load_return_byte_to_w(byte);
            self.store_w_to_temp_byte(function_symbol, dst_temp, byte);
        }
    }

    /// Shifts one frame-resident scalar left by one bit using PIC16 rotate-through-carry.
    fn shift_current_frame_value_left(&mut self, offset: u16, ty: Type) {
        self.program.push(AsmLine::Instr(AsmInstr::Bcf {
            f: low7(STATUS_ADDR),
            b: STATUS_C_BIT,
        }));
        for byte in 0..ty.byte_width() {
            self.save_carry_flag();
            self.prepare_pointer_from_pair(self.layout.helpers.frame_ptr, offset + byte as u16);
            self.restore_carry_flag();
            self.select_bank(INDF_ADDR);
            self.program.push(AsmLine::Instr(AsmInstr::Rlf {
                f: low7(INDF_ADDR),
                d: Dest::F,
            }));
        }
    }

    /// Rotates one frame-resident scalar left by one bit, preserving incoming carry.
    fn rotate_current_frame_value_left(&mut self, offset: u16, ty: Type) {
        for byte in 0..ty.byte_width() {
            self.save_carry_flag();
            self.prepare_pointer_from_pair(self.layout.helpers.frame_ptr, offset + byte as u16);
            self.restore_carry_flag();
            self.select_bank(INDF_ADDR);
            self.program.push(AsmLine::Instr(AsmInstr::Rlf {
                f: low7(INDF_ADDR),
                d: Dest::F,
            }));
        }
    }

    /// Shifts one frame-resident scalar right by one bit, arithmetic when requested.
    fn shift_current_frame_value_right(&mut self, offset: u16, ty: Type, arithmetic: bool) {
        self.program.push(AsmLine::Instr(AsmInstr::Bcf {
            f: low7(STATUS_ADDR),
            b: STATUS_C_BIT,
        }));
        if arithmetic {
            self.prepare_pointer_from_pair(
                self.layout.helpers.frame_ptr,
                offset + (ty.byte_width() - 1) as u16,
            );
            self.select_bank(INDF_ADDR);
            self.program.push(AsmLine::Instr(AsmInstr::Btfsc {
                f: low7(INDF_ADDR),
                b: 7,
            }));
            self.program.push(AsmLine::Instr(AsmInstr::Bsf {
                f: low7(STATUS_ADDR),
                b: STATUS_C_BIT,
            }));
        }
        for byte in (0..ty.byte_width()).rev() {
            self.save_carry_flag();
            self.prepare_pointer_from_pair(self.layout.helpers.frame_ptr, offset + byte as u16);
            self.restore_carry_flag();
            self.select_bank(INDF_ADDR);
            self.program.push(AsmLine::Instr(AsmInstr::Rrf {
                f: low7(INDF_ADDR),
                d: Dest::F,
            }));
        }
    }

    /// Applies a byte-wise binary instruction template across all bytes of a value.
    fn emit_per_byte_binary<F>(
        &mut self,
        function_symbol: SymbolId,
        lhs: Operand,
        rhs: Operand,
        ty: Type,
        dst_temp: usize,
        mut instr_for_addr: F,
    ) where
        F: FnMut(&mut Self, u8) -> AsmInstr,
    {
        for byte in 0..ty.byte_width() {
            self.load_operand_byte_to_w(function_symbol, lhs, ty, byte);
            self.store_w_to_addr(self.layout.helpers.scratch0);
            self.load_operand_byte_to_w(function_symbol, rhs, ty, byte);
            self.select_bank(self.layout.helpers.scratch0);
            let instr = instr_for_addr(self, low7(self.layout.helpers.scratch0));
            self.program.push(AsmLine::Instr(instr));
            self.store_w_to_temp_byte(function_symbol, dst_temp, byte);
        }
    }

    /// Applies a byte-wise unary emission callback across all bytes of a value.
    fn emit_per_byte_unary<F>(
        &mut self,
        function_symbol: SymbolId,
        src: Operand,
        ty: Type,
        dst_temp: usize,
        mut emit_for_byte: F,
    ) where
        F: FnMut(&mut Self, SymbolId, Operand, Type, usize),
    {
        for byte in 0..ty.byte_width() {
            emit_for_byte(self, function_symbol, src, ty, byte);
            self.store_w_to_temp_byte(function_symbol, dst_temp, byte);
        }
    }

    /// Lowers an explicit cast between supported integer widths and signedness modes.
    fn emit_cast(
        &mut self,
        function_symbol: SymbolId,
        src: Operand,
        src_ty: Type,
        kind: CastKind,
        dst_ty: Type,
        dst_temp: usize,
    ) {
        match kind {
            CastKind::Bitcast => self.copy_operand_to_temp(function_symbol, src, dst_ty, dst_temp),
            CastKind::F32ToQ16
            | CastKind::Q16ToF32
            | CastKind::I32ToF32
            | CastKind::U32ToF32
            | CastKind::F32ToI32
            | CastKind::F32ToU32 => {
                let helper = runtime_helper_for_cast(kind).expect("float cast helper exists");
                self.emit_runtime_cast_call(function_symbol, src, src_ty, helper, dst_ty, dst_temp);
            }
            CastKind::Truncate => {
                for byte in 0..dst_ty.byte_width() {
                    self.load_operand_byte_to_w(function_symbol, src, src_ty, byte);
                    self.store_w_to_temp_byte(function_symbol, dst_temp, byte);
                }
            }
            CastKind::ZeroExtend => {
                for byte in 0..src_ty.byte_width() {
                    self.load_operand_byte_to_w(function_symbol, src, src_ty, byte);
                    self.store_w_to_temp_byte(function_symbol, dst_temp, byte);
                }
                for byte in src_ty.byte_width()..dst_ty.byte_width() {
                    self.emit_const_to_w(0);
                    self.store_w_to_temp_byte(function_symbol, dst_temp, byte);
                }
            }
            CastKind::SignExtend => {
                for byte in 0..src_ty.byte_width() {
                    self.load_operand_byte_to_w(function_symbol, src, src_ty, byte);
                    if byte + 1 == src_ty.byte_width() {
                        self.store_w_to_addr(self.layout.helpers.scratch0);
                        self.load_addr_to_w(self.layout.helpers.scratch0);
                    }
                    self.store_w_to_temp_byte(function_symbol, dst_temp, byte);
                }
                if src_ty.byte_width() < dst_ty.byte_width() {
                    let negative = self.unique_label("sext_neg");
                    let end = self.unique_label("sext_end");
                    self.select_bank(self.layout.helpers.scratch0);
                    self.branch_if_bit_set(low7(self.layout.helpers.scratch0), 7, &negative);
                    for byte in src_ty.byte_width()..dst_ty.byte_width() {
                        self.emit_const_to_w(0);
                        self.store_w_to_temp_byte(function_symbol, dst_temp, byte);
                    }
                    self.jump_to_label(&end);
                    self.program.push(AsmLine::Label(negative));
                    for byte in src_ty.byte_width()..dst_ty.byte_width() {
                        self.emit_const_to_w(0xFF);
                        self.store_w_to_temp_byte(function_symbol, dst_temp, byte);
                    }
                    self.program.push(AsmLine::Label(end));
                }
            }
        }
    }

    /// Emits one single-argument runtime cast helper under the stack-first ABI.
    fn emit_runtime_cast_call(
        &mut self,
        function_symbol: SymbolId,
        src: Operand,
        src_ty: Type,
        helper: RuntimeHelper,
        dst_ty: Type,
        dst_temp: usize,
    ) {
        let info = helper.info();
        self.used_helpers.insert(helper);
        self.emit_stack_growth_check(
            info.arg_bytes,
            &format!("runtime helper {} argument", info.label),
        );
        self.push_operand(function_symbol, src, src_ty);
        self.program
            .push(AsmLine::Instr(AsmInstr::SetPage(info.label.to_string())));
        self.program
            .push(AsmLine::Instr(AsmInstr::Call(info.label.to_string())));
        self.restore_code_page_after_call();
        self.store_w_to_addr(self.layout.helpers.w_save);
        self.add_immediate_to_pair(self.layout.helpers.stack_ptr, negate_u16(info.arg_bytes));
        self.load_addr_to_w(self.layout.helpers.w_save);
        self.store_w_to_temp_byte(function_symbol, dst_temp, 0);
        for byte in 1..dst_ty.byte_width() {
            self.load_return_byte_to_w(byte);
            self.store_w_to_temp_byte(function_symbol, dst_temp, byte);
        }
    }

    /// Copies an operand into one frame-scoped temporary slot.
    fn copy_operand_to_temp(
        &mut self,
        function_symbol: SymbolId,
        src: Operand,
        ty: Type,
        temp: usize,
    ) {
        for byte in 0..ty.byte_width() {
            self.load_operand_byte_to_w(function_symbol, src, ty, byte);
            self.store_w_to_temp_byte(function_symbol, temp, byte);
        }
    }

    /// Copies an operand into one symbol storage location, absolute or frame-relative.
    fn copy_operand_to_symbol(
        &mut self,
        function_symbol: SymbolId,
        src: Operand,
        ty: Type,
        symbol: SymbolId,
    ) {
        match self.symbol_storage(symbol) {
            SymbolStorage::Absolute(base) => {
                self.copy_operand_to_slot(function_symbol, src, ty, base)
            }
            SymbolStorage::Frame(offset) => {
                for byte in 0..ty.byte_width() {
                    self.load_operand_byte_to_w(function_symbol, src, ty, byte);
                    self.store_w_to_frame_byte(function_symbol, offset + byte as u16);
                }
            }
        }
    }

    /// Copies an operand into any RAM slot, respecting 8-bit or 16-bit width.
    fn copy_operand_to_slot(
        &mut self,
        function_symbol: SymbolId,
        src: Operand,
        ty: Type,
        dst_base: u16,
    ) {
        for byte in 0..ty.byte_width() {
            self.load_operand_byte_to_w(function_symbol, src, ty, byte);
            self.store_w_to_addr(dst_base + byte as u16);
        }
    }

    /// Materializes the address of one symbol into a frame-scoped temp slot.
    fn emit_address_of_symbol(
        &mut self,
        function_symbol: SymbolId,
        symbol: SymbolId,
        dst_ty: Type,
        dst_temp: usize,
    ) {
        match self.symbol_storage(symbol) {
            SymbolStorage::Absolute(base) => {
                self.emit_const_to_w(low_byte(i64::from(base), dst_ty));
                self.store_w_to_temp_byte(function_symbol, dst_temp, 0);
                if dst_ty.byte_width() == 2 {
                    self.emit_const_to_w(high_byte(i64::from(base), dst_ty));
                    self.store_w_to_temp_byte(function_symbol, dst_temp, 1);
                }
            }
            SymbolStorage::Frame(offset) => {
                self.copy_pair_with_signed_offset(
                    self.layout.helpers.frame_ptr,
                    RegisterPair {
                        lo: self.layout.helpers.scratch0,
                        hi: self.layout.helpers.scratch1,
                    },
                    offset,
                );
                self.load_addr_to_w(self.layout.helpers.scratch0);
                self.store_w_to_addr(self.layout.helpers.w_save);
                if dst_ty.byte_width() == 2 {
                    self.load_addr_to_w(self.layout.helpers.scratch1);
                    self.store_w_to_temp_byte(function_symbol, dst_temp, 1);
                }
                self.load_addr_to_w(self.layout.helpers.w_save);
                self.store_w_to_temp_byte(function_symbol, dst_temp, 0);
            }
        }
    }

    /// Emits one function prologue that establishes the Phase 4 software frame pointer.
    fn emit_prologue(&mut self, function_symbol: SymbolId) {
        let arg_bytes = self.frame_layout(function_symbol).arg_bytes;
        let frame_bytes = self.frame_layout(function_symbol).frame_bytes;
        self.emit_stack_growth_check(
            frame_bytes,
            &format!("frame allocation for {}", self.symbol_name(function_symbol)),
        );
        self.load_addr_to_w(self.layout.helpers.frame_ptr.lo);
        self.store_w_to_addr(self.layout.helpers.scratch0);
        self.load_addr_to_w(self.layout.helpers.frame_ptr.hi);
        self.store_w_to_addr(self.layout.helpers.scratch1);
        self.copy_pair_with_signed_offset(
            self.layout.helpers.stack_ptr,
            self.layout.helpers.frame_ptr,
            negate_u16(arg_bytes),
        );
        self.load_addr_to_w(self.layout.helpers.scratch0);
        self.push_w();
        self.load_addr_to_w(self.layout.helpers.scratch1);
        self.push_w();
        let frame_storage_bytes = frame_bytes - 2;
        if frame_storage_bytes != 0 {
            self.add_immediate_to_pair(self.layout.helpers.stack_ptr, frame_storage_bytes);
        }
    }

    /// Emits one function epilogue that drops locals/temps, then restores caller FP.
    fn emit_epilogue(&mut self, function_symbol: SymbolId) {
        let saved_fp_offset = self.frame_layout(function_symbol).saved_fp_offset;
        let arg_bytes = self.frame_layout(function_symbol).arg_bytes;
        self.store_w_to_addr(self.layout.helpers.w_save);
        self.load_frame_byte_to_w(function_symbol, saved_fp_offset);
        self.store_w_to_addr(self.layout.helpers.scratch0);
        self.load_frame_byte_to_w(function_symbol, saved_fp_offset + 1);
        self.store_w_to_addr(self.layout.helpers.scratch1);
        self.copy_pair_with_signed_offset(
            self.layout.helpers.frame_ptr,
            self.layout.helpers.stack_ptr,
            arg_bytes,
        );
        self.load_addr_to_w(self.layout.helpers.scratch0);
        self.store_w_to_addr(self.layout.helpers.frame_ptr.lo);
        self.load_addr_to_w(self.layout.helpers.scratch1);
        self.store_w_to_addr(self.layout.helpers.frame_ptr.hi);
        self.load_addr_to_w(self.layout.helpers.w_save);
    }

    /// Emits the conservative Phase 6 ISR prologue before the shared frame logic runs.
    fn emit_interrupt_prologue(&mut self, function_symbol: SymbolId) {
        let Some(ctx) = self.layout.interrupt else {
            return;
        };

        self.store_w_to_shared_addr(ctx.w);
        self.load_direct_addr_to_w(STATUS_ADDR);
        self.store_w_to_shared_addr(ctx.status);
        self.load_direct_addr_to_w(PCLATH_ADDR);
        self.store_w_to_shared_addr(ctx.pclath);
        self.load_direct_addr_to_w(FSR_ADDR);
        self.store_w_to_shared_addr(ctx.fsr);

        self.current_bank = UNKNOWN_BANK;
        self.load_addr_to_w(self.layout.helpers.return_high);
        self.store_w_to_shared_addr(ctx.return_high);
        self.load_addr_to_w(self.layout.helpers.return_upper0);
        self.store_w_to_shared_addr(ctx.return_upper0);
        self.load_addr_to_w(self.layout.helpers.return_upper1);
        self.store_w_to_shared_addr(ctx.return_upper1);
        self.load_addr_to_w(self.layout.helpers.scratch0);
        self.store_w_to_shared_addr(ctx.scratch0);
        self.load_addr_to_w(self.layout.helpers.scratch1);
        self.store_w_to_shared_addr(ctx.scratch1);
        self.load_addr_to_w(self.layout.helpers.flag_save);
        self.store_w_to_shared_addr(ctx.flag_save);
        self.load_addr_to_w(self.layout.helpers.w_save);
        self.store_w_to_shared_addr(ctx.w_save);
        self.load_addr_to_w(self.layout.helpers.stack_ptr.lo);
        self.store_w_to_shared_addr(ctx.stack_ptr.lo);
        self.load_addr_to_w(self.layout.helpers.stack_ptr.hi);
        self.store_w_to_shared_addr(ctx.stack_ptr.hi);
        self.load_addr_to_w(self.layout.helpers.frame_ptr.lo);
        self.store_w_to_shared_addr(ctx.frame_ptr.lo);
        self.load_addr_to_w(self.layout.helpers.frame_ptr.hi);
        self.store_w_to_shared_addr(ctx.frame_ptr.hi);

        self.current_bank = UNKNOWN_BANK;
        self.emit_prologue(function_symbol);
    }

    /// Emits the Phase 6 ISR epilogue, restores saved context, and leaves `W` ready for `retfie`.
    fn emit_interrupt_epilogue(&mut self, function_symbol: SymbolId) {
        let Some(ctx) = self.layout.interrupt else {
            return;
        };

        self.emit_epilogue(function_symbol);
        self.current_bank = UNKNOWN_BANK;

        self.load_shared_addr_to_w(ctx.return_high);
        self.store_w_to_addr(self.layout.helpers.return_high);
        self.load_shared_addr_to_w(ctx.return_upper0);
        self.store_w_to_addr(self.layout.helpers.return_upper0);
        self.load_shared_addr_to_w(ctx.return_upper1);
        self.store_w_to_addr(self.layout.helpers.return_upper1);
        self.load_shared_addr_to_w(ctx.scratch0);
        self.store_w_to_addr(self.layout.helpers.scratch0);
        self.load_shared_addr_to_w(ctx.scratch1);
        self.store_w_to_addr(self.layout.helpers.scratch1);
        self.load_shared_addr_to_w(ctx.flag_save);
        self.store_w_to_addr(self.layout.helpers.flag_save);
        self.load_shared_addr_to_w(ctx.w_save);
        self.store_w_to_addr(self.layout.helpers.w_save);
        self.load_shared_addr_to_w(ctx.stack_ptr.lo);
        self.store_w_to_addr(self.layout.helpers.stack_ptr.lo);
        self.load_shared_addr_to_w(ctx.stack_ptr.hi);
        self.store_w_to_addr(self.layout.helpers.stack_ptr.hi);
        self.load_shared_addr_to_w(ctx.frame_ptr.lo);
        self.store_w_to_addr(self.layout.helpers.frame_ptr.lo);
        self.load_shared_addr_to_w(ctx.frame_ptr.hi);
        self.store_w_to_addr(self.layout.helpers.frame_ptr.hi);

        self.load_shared_addr_to_w(ctx.fsr);
        self.store_w_to_direct_addr(FSR_ADDR);
        self.load_shared_addr_to_w(ctx.pclath);
        self.store_w_to_direct_addr(PCLATH_ADDR);
        self.load_shared_addr_to_w(ctx.status);
        self.store_w_to_direct_addr(STATUS_ADDR);
        self.program.push(AsmLine::Instr(AsmInstr::Swapf {
            f: low7(ctx.w),
            d: Dest::F,
        }));
        self.program.push(AsmLine::Instr(AsmInstr::Swapf {
            f: low7(ctx.w),
            d: Dest::W,
        }));
    }

    /// Pushes one 8-bit or 16-bit operand onto the upward-growing software stack.
    fn push_operand(&mut self, function_symbol: SymbolId, operand: Operand, ty: Type) {
        for byte in 0..ty.byte_width() {
            self.load_operand_byte_to_w(function_symbol, operand, ty, byte);
            self.push_w();
        }
    }

    /// Emits one Phase 18 runtime stack bound check before stack growth.
    fn emit_stack_growth_check(&mut self, bytes: u16, reason: &str) {
        if !self.options.stack_check || bytes == 0 {
            return;
        }

        let ok_label = self.unique_label("stack_ok");
        self.program.push(AsmLine::Comment(format!(
            "phase18 stack check +{bytes} before {reason}"
        )));
        self.copy_pair_with_signed_offset(
            self.layout.helpers.stack_ptr,
            RegisterPair {
                lo: self.layout.helpers.scratch0,
                hi: self.layout.helpers.scratch1,
            },
            bytes,
        );

        self.emit_const_to_w(high_byte(
            i64::from(self.layout.stack_limit),
            Type::new(ScalarType::U16),
        ));
        self.select_bank(self.layout.helpers.scratch1);
        self.program.push(AsmLine::Instr(AsmInstr::Subwf {
            f: low7(self.layout.helpers.scratch1),
            d: Dest::W,
        }));
        self.branch_if_bit_clear(low7(STATUS_ADDR), STATUS_C_BIT, &ok_label);
        self.branch_if_bit_clear(low7(STATUS_ADDR), STATUS_Z_BIT, "__stack_overflow_trap");

        self.emit_const_to_w(low_byte(
            i64::from(self.layout.stack_limit),
            Type::new(ScalarType::U16),
        ));
        self.select_bank(self.layout.helpers.scratch0);
        self.program.push(AsmLine::Instr(AsmInstr::Subwf {
            f: low7(self.layout.helpers.scratch0),
            d: Dest::W,
        }));
        self.branch_if_bit_clear(low7(STATUS_ADDR), STATUS_C_BIT, &ok_label);
        self.branch_if_bit_set(low7(STATUS_ADDR), STATUS_Z_BIT, &ok_label);
        self.jump_to_label("__stack_overflow_trap");
        self.program.push(AsmLine::Label(ok_label));
    }

    /// Pushes the current `W` byte to the stack top and advances `SP`.
    fn push_w(&mut self) {
        self.store_w_to_addr(self.layout.helpers.scratch0);
        self.prepare_pointer_from_pair(self.layout.helpers.stack_ptr, 0);
        self.load_addr_to_w(self.layout.helpers.scratch0);
        self.store_w_to_indirect();
        self.add_immediate_to_pair(self.layout.helpers.stack_ptr, 1);
    }

    /// Copies one 16-bit helper pair into another while applying a signed constant offset.
    fn copy_pair_with_signed_offset(&mut self, src: RegisterPair, dst: RegisterPair, delta: u16) {
        let delta_ty = Type::new(ScalarType::U16);
        self.load_addr_to_w(src.lo);
        self.program.push(AsmLine::Instr(AsmInstr::Addlw(low_byte(
            i64::from(delta),
            delta_ty,
        ))));
        self.store_w_to_addr(dst.lo);
        self.load_addr_to_w(src.hi);
        self.program.push(AsmLine::Instr(AsmInstr::Btfsc {
            f: low7(STATUS_ADDR),
            b: STATUS_C_BIT,
        }));
        self.program.push(AsmLine::Instr(AsmInstr::Addlw(1)));
        let high = high_byte(i64::from(delta), delta_ty);
        if high != 0 {
            self.program.push(AsmLine::Instr(AsmInstr::Addlw(high)));
        }
        self.store_w_to_addr(dst.hi);
    }

    /// Adds a small constant delta to one 16-bit helper pair in place.
    fn add_immediate_to_pair(&mut self, pair: RegisterPair, delta: u16) {
        self.copy_pair_with_signed_offset(pair, pair, delta);
    }

    /// Loads a stack-frame byte addressed by `FP + offset` into `W`.
    fn load_frame_byte_to_w(&mut self, _function_symbol: SymbolId, offset: u16) {
        self.load_current_frame_byte_to_w(offset);
    }

    /// Stores `W` into a stack-frame byte addressed by `FP + offset`.
    fn store_w_to_frame_byte(&mut self, _function_symbol: SymbolId, offset: u16) {
        self.store_w_to_current_frame_byte(offset);
    }

    /// Loads one byte from the active frame at `FP + offset` into `W`.
    fn load_current_frame_byte_to_w(&mut self, offset: u16) {
        self.save_carry_flag();
        self.prepare_pointer_from_pair(self.layout.helpers.frame_ptr, offset);
        self.load_indirect_to_w();
        self.restore_carry_flag();
    }

    /// Stores `W` into one byte of the active frame at `FP + offset`.
    fn store_w_to_current_frame_byte(&mut self, offset: u16) {
        self.store_w_to_addr(self.layout.helpers.scratch0);
        self.save_status_flags();
        self.prepare_pointer_from_pair(self.layout.helpers.frame_ptr, offset);
        self.load_addr_to_w(self.layout.helpers.scratch0);
        self.store_w_to_indirect();
        self.restore_status_flags();
    }

    /// Sets one bit in the active frame through `INDF`.
    fn set_current_frame_bit(&mut self, offset: u16, bit: u8) {
        self.prepare_pointer_from_pair(self.layout.helpers.frame_ptr, offset);
        self.select_bank(INDF_ADDR);
        self.program.push(AsmLine::Instr(AsmInstr::Bsf {
            f: low7(INDF_ADDR),
            b: bit,
        }));
    }

    /// Branches on one active-frame bit using PIC16 skip semantics.
    fn branch_on_current_frame_bit(
        &mut self,
        offset: u16,
        bit: u8,
        set_label: &str,
        clear_label: &str,
    ) {
        self.prepare_pointer_from_pair(self.layout.helpers.frame_ptr, offset);
        self.select_bank(INDF_ADDR);
        self.branch_if_bit_set(low7(INDF_ADDR), bit, set_label);
        self.jump_to_label(clear_label);
    }

    /// Clears one scalar slot that lives inside the active call frame.
    fn clear_current_frame_slot(&mut self, offset: u16, ty: Type) {
        for byte in 0..ty.byte_width() {
            self.emit_const_to_w(0);
            self.store_w_to_current_frame_byte(offset + byte as u16);
        }
    }

    fn store_i32_const_to_current_frame(&mut self, offset: u16, value: i64) {
        let ty = Type::new(ScalarType::I32);
        for byte in 0..4u16 {
            self.emit_const_to_w(value_byte(value, ty, byte as usize));
            self.store_w_to_current_frame_byte(offset + byte);
        }
    }

    /// Branches on whether one active-frame scalar value is zero or non-zero.
    fn emit_current_frame_nonzero_branch(
        &mut self,
        offset: u16,
        ty: Type,
        then_label: &str,
        else_label: &str,
    ) {
        for byte in 0..ty.byte_width() {
            self.load_current_frame_byte_to_w(offset + byte as u16);
            self.branch_if_bit_clear(low7(STATUS_ADDR), STATUS_Z_BIT, then_label);
        }
        self.jump_to_label(else_label);
    }

    /// Branches on whether two active-frame scalar values are byte-exact equal.
    fn emit_current_frame_equal_branch(
        &mut self,
        lhs_offset: u16,
        rhs_offset: u16,
        ty: Type,
        equal_label: &str,
        not_equal_label: &str,
    ) {
        for byte in 0..ty.byte_width() {
            self.compare_current_frame_byte(lhs_offset, rhs_offset, byte);
            self.branch_if_bit_clear(low7(STATUS_ADDR), STATUS_Z_BIT, not_equal_label);
        }
        self.jump_to_label(equal_label);
    }

    /// Subtracts one active-frame byte from another and leaves compare flags live.
    fn compare_current_frame_byte(&mut self, lhs_offset: u16, rhs_offset: u16, byte_index: usize) {
        self.load_current_frame_byte_to_w(lhs_offset + byte_index as u16);
        self.store_w_to_addr(self.layout.helpers.scratch0);
        self.load_current_frame_byte_to_w(rhs_offset + byte_index as u16);
        self.select_bank(self.layout.helpers.scratch0);
        self.program.push(AsmLine::Instr(AsmInstr::Subwf {
            f: low7(self.layout.helpers.scratch0),
            d: Dest::W,
        }));
    }

    /// Branches when `lhs >= rhs` using unsigned compare semantics over the active frame.
    fn emit_current_frame_unsigned_ge_branch(
        &mut self,
        lhs_offset: u16,
        rhs_offset: u16,
        ty: Type,
        ge_label: &str,
        lt_label: &str,
    ) {
        for byte in (0..ty.byte_width()).rev() {
            self.compare_current_frame_byte(lhs_offset, rhs_offset, byte);
            if byte != 0 {
                let next_label = self.unique_label("rt_cmp_next");
                self.branch_if_bit_set(low7(STATUS_ADDR), STATUS_Z_BIT, &next_label);
                self.branch_if_bit_set(low7(STATUS_ADDR), STATUS_C_BIT, ge_label);
                self.jump_to_label(lt_label);
                self.program.push(AsmLine::Label(next_label));
            } else {
                self.branch_if_bit_set(low7(STATUS_ADDR), STATUS_C_BIT, ge_label);
                self.jump_to_label(lt_label);
            }
        }
    }

    /// Adds one active-frame scalar into another slot in place.
    fn add_current_frame_value_into_slot(&mut self, src_offset: u16, dst_offset: u16, ty: Type) {
        for byte in 0..ty.byte_width() {
            self.load_current_frame_byte_to_w(src_offset + byte as u16);
            self.store_w_to_addr(self.layout.helpers.scratch0);
            self.load_current_frame_byte_to_w(dst_offset + byte as u16);
            if byte != 0 {
                self.clear_addr(self.layout.helpers.scratch1);
                self.program.push(AsmLine::Instr(AsmInstr::Btfsc {
                    f: low7(STATUS_ADDR),
                    b: STATUS_C_BIT,
                }));
                self.program.push(AsmLine::Instr(AsmInstr::Addlw(1)));
                self.program.push(AsmLine::Instr(AsmInstr::Btfsc {
                    f: low7(STATUS_ADDR),
                    b: STATUS_C_BIT,
                }));
                self.program.push(AsmLine::Instr(AsmInstr::Bsf {
                    f: low7(self.layout.helpers.scratch1),
                    b: 0,
                }));
            }
            self.store_w_to_addr(self.layout.helpers.w_save);
            self.load_addr_to_w(self.layout.helpers.scratch0);
            self.select_bank(self.layout.helpers.w_save);
            self.program.push(AsmLine::Instr(AsmInstr::Addwf {
                f: low7(self.layout.helpers.w_save),
                d: Dest::W,
            }));
            if byte != 0 {
                self.select_bank(self.layout.helpers.scratch1);
                self.restore_code_page_after_call();
                self.program.push(AsmLine::Instr(AsmInstr::Btfsc {
                    f: low7(self.layout.helpers.scratch1),
                    b: 0,
                }));
                self.program.push(AsmLine::Instr(AsmInstr::Bsf {
                    f: low7(STATUS_ADDR),
                    b: STATUS_C_BIT,
                }));
            }
            self.store_w_to_current_frame_byte(dst_offset + byte as u16);
        }
    }

    /// Subtracts one active-frame scalar from another slot in place.
    fn sub_current_frame_value_from_slot(&mut self, src_offset: u16, dst_offset: u16, ty: Type) {
        for byte in 0..ty.byte_width() {
            self.load_current_frame_byte_to_w(dst_offset + byte as u16);
            self.store_w_to_addr(self.layout.helpers.scratch0);
            self.load_current_frame_byte_to_w(src_offset + byte as u16);
            if byte != 0 {
                let no_borrow = self.unique_label("rt_sub_no_borrow");
                self.clear_addr(self.layout.helpers.scratch1);
                self.branch_if_bit_set(low7(STATUS_ADDR), STATUS_C_BIT, &no_borrow);
                self.program.push(AsmLine::Instr(AsmInstr::Addlw(1)));
                self.program.push(AsmLine::Instr(AsmInstr::Btfsc {
                    f: low7(STATUS_ADDR),
                    b: STATUS_C_BIT,
                }));
                self.program.push(AsmLine::Instr(AsmInstr::Bsf {
                    f: low7(self.layout.helpers.scratch1),
                    b: 0,
                }));
                self.program.push(AsmLine::Label(no_borrow));
            }
            self.store_w_to_addr(self.layout.helpers.w_save);
            self.load_addr_to_w(self.layout.helpers.w_save);
            self.select_bank(self.layout.helpers.scratch0);
            self.program.push(AsmLine::Instr(AsmInstr::Subwf {
                f: low7(self.layout.helpers.scratch0),
                d: Dest::W,
            }));
            if byte != 0 {
                self.select_bank(self.layout.helpers.scratch1);
                self.program.push(AsmLine::Instr(AsmInstr::Btfsc {
                    f: low7(self.layout.helpers.scratch1),
                    b: 0,
                }));
                self.program.push(AsmLine::Instr(AsmInstr::Bcf {
                    f: low7(STATUS_ADDR),
                    b: STATUS_C_BIT,
                }));
            }
            self.store_w_to_current_frame_byte(dst_offset + byte as u16);
        }
    }

    /// Negates one active-frame scalar in place with two's-complement wrap semantics.
    fn negate_current_frame_value(&mut self, offset: u16, ty: Type) {
        for byte in 0..ty.byte_width() {
            self.clear_addr(self.layout.helpers.scratch0);
            self.load_current_frame_byte_to_w(offset + byte as u16);
            if byte != 0 {
                let no_borrow = self.unique_label("rt_neg_no_borrow");
                self.clear_addr(self.layout.helpers.scratch1);
                self.branch_if_bit_set(low7(STATUS_ADDR), STATUS_C_BIT, &no_borrow);
                self.program.push(AsmLine::Instr(AsmInstr::Addlw(1)));
                self.program.push(AsmLine::Instr(AsmInstr::Btfsc {
                    f: low7(STATUS_ADDR),
                    b: STATUS_C_BIT,
                }));
                self.program.push(AsmLine::Instr(AsmInstr::Bsf {
                    f: low7(self.layout.helpers.scratch1),
                    b: 0,
                }));
                self.program.push(AsmLine::Label(no_borrow));
            }
            self.store_w_to_addr(self.layout.helpers.w_save);
            self.load_addr_to_w(self.layout.helpers.w_save);
            self.select_bank(self.layout.helpers.scratch0);
            self.program.push(AsmLine::Instr(AsmInstr::Subwf {
                f: low7(self.layout.helpers.scratch0),
                d: Dest::W,
            }));
            if byte != 0 {
                self.select_bank(self.layout.helpers.scratch1);
                self.program.push(AsmLine::Instr(AsmInstr::Btfsc {
                    f: low7(self.layout.helpers.scratch1),
                    b: 0,
                }));
                self.program.push(AsmLine::Instr(AsmInstr::Bcf {
                    f: low7(STATUS_ADDR),
                    b: STATUS_C_BIT,
                }));
            }
            self.store_w_to_current_frame_byte(offset + byte as u16);
        }
    }

    /// Decrements one active-frame scalar in place.
    fn decrement_current_frame_value(&mut self, offset: u16, ty: Type) {
        for byte in 0..ty.byte_width() {
            self.load_current_frame_byte_to_w(offset + byte as u16);
            if byte == 0 {
                self.program.push(AsmLine::Instr(AsmInstr::Addlw(0xFF)));
            } else {
                self.program.push(AsmLine::Instr(AsmInstr::Btfss {
                    f: low7(STATUS_ADDR),
                    b: STATUS_C_BIT,
                }));
                self.program.push(AsmLine::Instr(AsmInstr::Addlw(0xFF)));
            }
            self.store_w_to_current_frame_byte(offset + byte as u16);
        }
    }

    /// Places one active-frame scalar into the ABI return locations.
    fn emit_return_current_frame_value(&mut self, offset: u16, ty: Type) {
        for byte in 1..ty.byte_width() {
            self.load_current_frame_byte_to_w(offset + byte as u16);
            self.store_w_to_return_byte(byte);
        }
        self.load_current_frame_byte_to_w(offset);
    }

    /// Programs `FSR/IRP` from one helper pair plus a constant byte offset.
    fn prepare_pointer_from_pair(&mut self, pair: RegisterPair, byte_offset: u16) {
        let offset_ty = Type::new(ScalarType::U16);
        self.load_addr_to_w(pair.lo);
        let low = low_byte(i64::from(byte_offset), offset_ty);
        self.program.push(AsmLine::Instr(AsmInstr::Addlw(low)));
        self.store_w_to_addr(FSR_ADDR);

        self.load_addr_to_w(pair.hi);
        self.program.push(AsmLine::Instr(AsmInstr::Btfsc {
            f: low7(STATUS_ADDR),
            b: STATUS_C_BIT,
        }));
        self.program.push(AsmLine::Instr(AsmInstr::Addlw(1)));
        let high = high_byte(i64::from(byte_offset), offset_ty);
        if high != 0 {
            self.program.push(AsmLine::Instr(AsmInstr::Addlw(high)));
        }
        self.store_w_to_addr(self.layout.helpers.scratch1);
        self.set_irp_from_addr(self.layout.helpers.scratch1);
    }

    /// Loads one indirectly addressed scalar object through `FSR/INDF` into a temp slot.
    fn emit_indirect_load(
        &mut self,
        function_symbol: SymbolId,
        ptr: Operand,
        ty: Type,
        dst_temp: usize,
    ) {
        for byte in 0..ty.byte_width() {
            self.prepare_indirect_pointer(function_symbol, ptr, byte as u8);
            self.load_indirect_to_w();
            self.store_w_to_temp_byte(function_symbol, dst_temp, byte);
        }
    }

    /// Stores one scalar value through an indirect pointer using `FSR/INDF`.
    fn emit_indirect_store(
        &mut self,
        function_symbol: SymbolId,
        ptr: Operand,
        value: Operand,
        ty: Type,
    ) {
        for byte in 0..ty.byte_width() {
            self.load_operand_byte_to_w(function_symbol, value, ty, byte);
            self.store_w_to_addr(self.layout.helpers.scratch0);
            self.prepare_indirect_pointer(function_symbol, ptr, byte as u8);
            self.load_addr_to_w(self.layout.helpers.scratch0);
            self.store_w_to_indirect();
        }
    }

    /// Programs `FSR` and `STATUS.IRP` for one pointer plus a small byte offset.
    fn prepare_indirect_pointer(
        &mut self,
        function_symbol: SymbolId,
        ptr: Operand,
        byte_offset: u8,
    ) {
        let ptr_ty = Type::new(ScalarType::U16);
        self.load_operand_byte_to_w(function_symbol, ptr, ptr_ty, 0);
        if byte_offset != 0 {
            self.program
                .push(AsmLine::Instr(AsmInstr::Addlw(byte_offset)));
        }
        self.store_w_to_addr(self.layout.helpers.w_save);

        self.load_operand_byte_to_w(function_symbol, ptr, ptr_ty, 1);
        if byte_offset != 0 {
            self.program.push(AsmLine::Instr(AsmInstr::Btfsc {
                f: low7(STATUS_ADDR),
                b: STATUS_C_BIT,
            }));
            self.program.push(AsmLine::Instr(AsmInstr::Addlw(1)));
        }
        self.store_w_to_addr(self.layout.helpers.scratch1);
        self.set_irp_from_addr(self.layout.helpers.scratch1);
        self.load_addr_to_w(self.layout.helpers.w_save);
        self.store_w_to_addr(FSR_ADDR);
    }

    /// Loads the byte addressed by the current `FSR/IRP` pair into `W`.
    fn load_indirect_to_w(&mut self) {
        self.select_bank(INDF_ADDR);
        self.program.push(AsmLine::Instr(AsmInstr::Movf {
            f: low7(INDF_ADDR),
            d: Dest::W,
        }));
    }

    /// Stores `W` into the byte addressed by the current `FSR/IRP` pair.
    fn store_w_to_indirect(&mut self) {
        self.select_bank(INDF_ADDR);
        self.program
            .push(AsmLine::Instr(AsmInstr::Movwf(low7(INDF_ADDR))));
    }

    /// Saves carry and zero into one dedicated helper byte without disturbing `W`.
    fn save_status_flags(&mut self) {
        let status = low7(STATUS_ADDR);
        self.select_bank(self.layout.helpers.flag_save);
        self.program.push(AsmLine::Instr(AsmInstr::Bcf {
            f: low7(self.layout.helpers.flag_save),
            b: STATUS_C_BIT,
        }));
        self.program.push(AsmLine::Instr(AsmInstr::Btfsc {
            f: status,
            b: STATUS_C_BIT,
        }));
        self.program.push(AsmLine::Instr(AsmInstr::Bsf {
            f: low7(self.layout.helpers.flag_save),
            b: STATUS_C_BIT,
        }));
        self.program.push(AsmLine::Instr(AsmInstr::Bcf {
            f: low7(self.layout.helpers.flag_save),
            b: STATUS_Z_BIT,
        }));
        self.program.push(AsmLine::Instr(AsmInstr::Btfsc {
            f: status,
            b: STATUS_Z_BIT,
        }));
        self.program.push(AsmLine::Instr(AsmInstr::Bsf {
            f: low7(self.layout.helpers.flag_save),
            b: STATUS_Z_BIT,
        }));
    }

    /// Saves carry only, leaving later frame loads free to report their own zero state.
    fn save_carry_flag(&mut self) {
        let status = low7(STATUS_ADDR);
        self.select_bank(self.layout.helpers.flag_save);
        self.program.push(AsmLine::Instr(AsmInstr::Bcf {
            f: low7(self.layout.helpers.flag_save),
            b: STATUS_C_BIT,
        }));
        self.program.push(AsmLine::Instr(AsmInstr::Btfsc {
            f: status,
            b: STATUS_C_BIT,
        }));
        self.program.push(AsmLine::Instr(AsmInstr::Bsf {
            f: low7(self.layout.helpers.flag_save),
            b: STATUS_C_BIT,
        }));
    }

    /// Restores carry and zero from the helper flag-save byte without disturbing `W`.
    fn restore_status_flags(&mut self) {
        let status = low7(STATUS_ADDR);
        self.program.push(AsmLine::Instr(AsmInstr::Bcf {
            f: status,
            b: STATUS_C_BIT,
        }));
        self.program.push(AsmLine::Instr(AsmInstr::Bcf {
            f: status,
            b: STATUS_Z_BIT,
        }));
        self.select_bank(self.layout.helpers.flag_save);
        self.program.push(AsmLine::Instr(AsmInstr::Btfsc {
            f: low7(self.layout.helpers.flag_save),
            b: STATUS_C_BIT,
        }));
        self.program.push(AsmLine::Instr(AsmInstr::Bsf {
            f: status,
            b: STATUS_C_BIT,
        }));
        self.program.push(AsmLine::Instr(AsmInstr::Btfsc {
            f: low7(self.layout.helpers.flag_save),
            b: STATUS_Z_BIT,
        }));
        self.program.push(AsmLine::Instr(AsmInstr::Bsf {
            f: status,
            b: STATUS_Z_BIT,
        }));
    }

    /// Restores carry only, keeping `Z` from the most recent `movf`/test.
    fn restore_carry_flag(&mut self) {
        let status = low7(STATUS_ADDR);
        self.program.push(AsmLine::Instr(AsmInstr::Bcf {
            f: status,
            b: STATUS_C_BIT,
        }));
        self.select_bank(self.layout.helpers.flag_save);
        self.program.push(AsmLine::Instr(AsmInstr::Btfsc {
            f: low7(self.layout.helpers.flag_save),
            b: STATUS_C_BIT,
        }));
        self.program.push(AsmLine::Instr(AsmInstr::Bsf {
            f: status,
            b: STATUS_C_BIT,
        }));
    }

    /// Updates the indirect-bank select bit from a scratch byte that holds the pointer high byte.
    fn set_irp_from_addr(&mut self, addr: u16) {
        let status = low7(STATUS_ADDR);
        self.program.push(AsmLine::Instr(AsmInstr::Bcf {
            f: status,
            b: STATUS_IRP_BIT,
        }));
        self.select_bank(addr);
        self.program.push(AsmLine::Instr(AsmInstr::Btfsc {
            f: low7(addr),
            b: 0,
        }));
        self.program.push(AsmLine::Instr(AsmInstr::Bsf {
            f: status,
            b: STATUS_IRP_BIT,
        }));
    }

    /// Loads one operand byte into `W`, handling constants, symbols, and temps.
    fn load_operand_byte_to_w(
        &mut self,
        function_symbol: SymbolId,
        operand: Operand,
        ty: Type,
        byte_index: usize,
    ) {
        match operand {
            Operand::Constant(value) => {
                self.emit_const_to_w(value_byte(value, ty, byte_index));
            }
            Operand::Symbol(symbol) => match self.symbol_storage(symbol) {
                SymbolStorage::Absolute(base) => self.load_addr_to_w(base + byte_index as u16),
                SymbolStorage::Frame(offset) => {
                    self.load_frame_byte_to_w(function_symbol, offset + byte_index as u16)
                }
            },
            Operand::Temp(temp) => self.load_frame_byte_to_w(
                function_symbol,
                self.temp_offset(function_symbol, temp) + byte_index as u16,
            ),
        }
    }

    /// Loads one RAM address into `W` after selecting the correct bank.
    fn load_addr_to_w(&mut self, addr: u16) {
        self.select_bank(addr);
        self.program.push(AsmLine::Instr(AsmInstr::Movf {
            f: low7(addr),
            d: Dest::W,
        }));
    }

    /// Loads one mirrored/common address into `W` without touching bank bits.
    fn load_direct_addr_to_w(&mut self, addr: u16) {
        self.program.push(AsmLine::Instr(AsmInstr::Movf {
            f: low7(addr),
            d: Dest::W,
        }));
    }

    /// Loads one shared ISR-context byte into `W` without changing `STATUS`.
    fn load_shared_addr_to_w(&mut self, addr: u16) {
        self.load_direct_addr_to_w(addr);
    }

    /// Emits the shortest sequence to place an 8-bit constant into `W`.
    fn emit_const_to_w(&mut self, value: u8) {
        if value == 0 {
            self.program.push(AsmLine::Instr(AsmInstr::Clrw));
        } else {
            self.program.push(AsmLine::Instr(AsmInstr::Movlw(value)));
        }
    }

    /// Stores a constant value into an 8-bit or 16-bit RAM slot.
    fn store_const_value(&mut self, base: u16, ty: Type, value: i64) {
        for byte in 0..ty.byte_width() {
            self.emit_const_to_w(value_byte(value, ty, byte));
            self.store_w_to_addr(base + byte as u16);
        }
    }

    /// Stores the current `W` value into a banked RAM address.
    fn store_w_to_addr(&mut self, addr: u16) {
        self.select_bank(addr);
        self.program
            .push(AsmLine::Instr(AsmInstr::Movwf(low7(addr))));
        if addr == STATUS_ADDR {
            self.current_bank = UNKNOWN_BANK;
        }
    }

    /// Stores the current `W` value into a mirrored/common address without bank selection.
    fn store_w_to_direct_addr(&mut self, addr: u16) {
        self.program
            .push(AsmLine::Instr(AsmInstr::Movwf(low7(addr))));
        if addr == STATUS_ADDR {
            self.current_bank = UNKNOWN_BANK;
        }
    }

    /// Stores the current `W` value into a shared ISR-context byte without changing `STATUS`.
    fn store_w_to_shared_addr(&mut self, addr: u16) {
        self.store_w_to_direct_addr(addr);
    }

    /// Clears one banked RAM address to zero.
    fn clear_addr(&mut self, addr: u16) {
        self.select_bank(addr);
        self.program
            .push(AsmLine::Instr(AsmInstr::Clrf(low7(addr))));
    }

    /// Clears every byte that belongs to a value slot.
    fn clear_slot(&mut self, base: u16, ty: Type) {
        for byte in 0..ty.byte_width() {
            self.clear_addr(base + byte as u16);
        }
    }

    /// Stores `W` into one byte of a frame-scoped temporary.
    fn store_w_to_temp_byte(&mut self, function_symbol: SymbolId, temp: usize, byte_index: usize) {
        let offset = self.temp_offset(function_symbol, temp) + byte_index as u16;
        self.store_w_to_frame_byte(function_symbol, offset);
    }

    /// Clears a frame-scoped temporary to zero.
    fn clear_temp(&mut self, function_symbol: SymbolId, temp: usize, ty: Type) {
        for byte in 0..ty.byte_width() {
            self.emit_const_to_w(0);
            self.store_w_to_temp_byte(function_symbol, temp, byte);
        }
    }

    /// Emits a page-safe unconditional branch to a label.
    fn branch_to_label(&mut self, label: &str) {
        self.program
            .push(AsmLine::Instr(AsmInstr::SetPage(label.to_string())));
        self.program
            .push(AsmLine::Instr(AsmInstr::Goto(label.to_string())));
    }

    /// Emits an unconditional page-safe branch.
    fn jump_to_label(&mut self, label: &str) {
        self.branch_to_label(label);
    }

    /// Branches to `label` when the selected bit is set, otherwise falls through.
    fn branch_if_bit_set(&mut self, f: u8, b: u8, label: &str) {
        let after_label = self.unique_label("branch_after");
        self.program
            .push(AsmLine::Instr(AsmInstr::SetPage(after_label.clone())));
        self.program.push(AsmLine::Instr(AsmInstr::Btfss { f, b }));
        self.program
            .push(AsmLine::Instr(AsmInstr::Goto(after_label.clone())));
        self.branch_to_label(label);
        self.program.push(AsmLine::Label(after_label));
    }

    /// Branches to `label` when the selected bit is clear, otherwise falls through.
    fn branch_if_bit_clear(&mut self, f: u8, b: u8, label: &str) {
        let after_label = self.unique_label("branch_after");
        self.program
            .push(AsmLine::Instr(AsmInstr::SetPage(after_label.clone())));
        self.program.push(AsmLine::Instr(AsmInstr::Btfsc { f, b }));
        self.program
            .push(AsmLine::Instr(AsmInstr::Goto(after_label.clone())));
        self.branch_to_label(label);
        self.program.push(AsmLine::Label(after_label));
    }

    /// Restores PCLATH page bits to the next local instruction after a CALL returns.
    fn restore_code_page_after_call(&mut self) {
        let label = self.unique_label("after_call");
        self.program
            .push(AsmLine::Instr(AsmInstr::SetPage(label.clone())));
        self.program.push(AsmLine::Label(label));
    }

    /// Updates STATUS bank bits when an address lives outside the current bank.
    fn select_bank(&mut self, addr: u16) {
        let bank = ((addr >> 7) & 0x03) as u8;
        if bank == self.current_bank {
            return;
        }
        let status = low7(STATUS_ADDR);
        if self.current_bank == UNKNOWN_BANK || ((self.current_bank ^ bank) & 0x01) != 0 {
            if (bank & 0x01) == 0 {
                self.program
                    .push(AsmLine::Instr(AsmInstr::Bcf { f: status, b: 5 }));
            } else {
                self.program
                    .push(AsmLine::Instr(AsmInstr::Bsf { f: status, b: 5 }));
            }
        }
        if self.current_bank == UNKNOWN_BANK || ((self.current_bank ^ bank) & 0x02) != 0 {
            if (bank & 0x02) == 0 {
                self.program
                    .push(AsmLine::Instr(AsmInstr::Bcf { f: status, b: 6 }));
            } else {
                self.program
                    .push(AsmLine::Instr(AsmInstr::Bsf { f: status, b: 6 }));
            }
        }
        self.current_bank = bank;
    }

    /// Emits every internal arithmetic helper that codegen marked as used.
    fn emit_runtime_helpers(&mut self, diagnostics: &mut DiagnosticBag) {
        let mut helpers = self.used_helpers.clone();
        let mut changed = true;
        while changed {
            changed = false;
            for helper in helpers.clone() {
                for dependency in helper.dependencies_for_profiles(
                    self.options.runtime_profile,
                    self.options.math_profile,
                ) {
                    changed |= helpers.insert(*dependency);
                }
            }
        }
        let helpers = helpers.into_iter().collect::<Vec<_>>();
        if let Err(message) = validate_helper_dependency_graph(
            &helpers,
            self.options.runtime_profile,
            self.options.math_profile,
        ) {
            diagnostics.error("backend", None, message, None);
            return;
        }
        if !helpers.is_empty() {
            self.program.push(AsmLine::Comment(
                "runtime helpers (Phase 33 catalog/pruned section)".to_string(),
            ));
        }
        for helper in helpers {
            self.emit_runtime_helper(helper);
        }
    }

    /// Emits one infinite-loop trap when Phase 18 stack checks are enabled.
    fn emit_stack_overflow_trap(&mut self) {
        if !self.options.stack_check {
            return;
        }
        self.program
            .push(AsmLine::Label("__stack_overflow_trap".to_string()));
        self.program
            .push(AsmLine::Comment("phase18 stack overflow trap".to_string()));
        self.program.push(AsmLine::Instr(AsmInstr::Goto(
            "__stack_overflow_trap".to_string(),
        )));
    }

    /// Emits every Phase 14 ROM object as one callable RETLW table in program memory.
    fn emit_rom_objects(&mut self, diagnostics: &mut DiagnosticBag) {
        let placements = self.allocate_rom_tables(diagnostics);
        if diagnostics.has_errors() || placements.is_empty() {
            return;
        }

        self.program.push(AsmLine::Comment(
            "program-memory ROM tables (Phase 14)".to_string(),
        ));
        for placement in placements {
            let Some(global) = self
                .typed_program
                .globals
                .iter()
                .find(|global| global.symbol == placement.symbol)
            else {
                diagnostics.error(
                    "backend",
                    None,
                    "missing ROM global metadata during emission",
                    None,
                );
                continue;
            };
            let symbol = &self.typed_program.symbols[placement.symbol];
            let TypedGlobalInitializer::Bytes(bytes) = global
                .initializer
                .as_ref()
                .cloned()
                .unwrap_or(TypedGlobalInitializer::Bytes(Vec::new()))
            else {
                diagnostics.error(
                    "backend",
                    None,
                    format!(
                        "ROM object `{}` needs a constant byte payload initializer in phase 14",
                        symbol.name
                    ),
                    None,
                );
                continue;
            };
            self.program.push(AsmLine::Org(placement.start));
            self.program.push(AsmLine::Comment(format!(
                "ROM {} @0x{:04X} ({} element(s), {} byte payload as RETLW table)",
                format_rom_symbol_name(symbol),
                placement.start,
                symbol.ty.top_array_len().unwrap_or(0),
                bytes.len()
            )));
            self.program
                .push(AsmLine::Label(rom_object_label(symbol.id)));
            self.program.push(AsmLine::Instr(AsmInstr::Addwf {
                f: low7(self.target.sfr_address("PCL").unwrap_or(0x02)),
                d: Dest::F,
            }));
            for byte in bytes {
                self.program.push(AsmLine::Instr(AsmInstr::Retlw(byte)));
            }
        }
    }

    /// Allocates top-level ROM tables from high program memory without crossing one 256-word page.
    fn allocate_rom_tables(&self, diagnostics: &mut DiagnosticBag) -> Vec<RomTablePlacement> {
        let rom_globals = self
            .typed_program
            .globals
            .iter()
            .filter(|global| self.symbol_type(global.symbol).is_rom())
            .collect::<Vec<_>>();
        if rom_globals.is_empty() {
            return Vec::new();
        }

        let code_high = self.program_high_water_mark();
        let mut cursor = self.target.program_words;
        let mut placements = Vec::with_capacity(rom_globals.len());

        for global in rom_globals.into_iter().rev() {
            let symbol = &self.typed_program.symbols[global.symbol];
            let bytes = match &global.initializer {
                Some(TypedGlobalInitializer::Bytes(bytes)) => bytes,
                Some(TypedGlobalInitializer::Scalar(_))
                | Some(TypedGlobalInitializer::Address { .. })
                | None => {
                    diagnostics.error(
                        "backend",
                        None,
                        format!(
                            "ROM object `{}` needs a constant byte payload initializer in phase 14",
                            symbol.name
                        ),
                        None,
                    );
                    continue;
                }
            };

            let block_words = 1u16 + bytes.len() as u16;
            if block_words > 256 {
                diagnostics.error(
                    "backend",
                    None,
                    format!(
                        "ROM object `{}` is too large for one phase 14 RETLW page ({} data bytes)",
                        symbol.name,
                        bytes.len()
                    ),
                    Some("shrink the ROM object so one RETLW table fits within a single 256-word page".to_string()),
                );
                continue;
            }

            loop {
                if cursor == 0 {
                    diagnostics.error(
                        "backend",
                        None,
                        format!(
                            "program memory overflow: not enough program memory for ROM object `{}`",
                            symbol.name
                        ),
                        Some("reduce code size or ROM table size for this target".to_string()),
                    );
                    break;
                }
                let page_base = ((cursor - 1) / 256) * 256;
                let room = cursor - page_base;
                if room < block_words {
                    cursor = page_base;
                    continue;
                }
                let start = cursor - block_words;
                if start <= code_high {
                    diagnostics.error(
                        "backend",
                        None,
                        format!(
                            "ROM table/code overlap: ROM object `{}` would overlap generated code at 0x{:04X}",
                            symbol.name, start
                        ),
                        Some("reduce code size or shrink ROM data for this target".to_string()),
                    );
                    break;
                }
                placements.push(RomTablePlacement {
                    symbol: global.symbol,
                    start,
                });
                cursor = start;
                break;
            }
        }

        placements.sort_by_key(|placement| placement.start);
        placements
    }

    /// Returns the highest encoded word address currently occupied by emitted code.
    fn program_high_water_mark(&self) -> u16 {
        let mut pc = 0u16;
        let mut high = 0u16;
        for line in &self.program.lines {
            match line {
                AsmLine::Org(addr) => pc = *addr,
                AsmLine::Instr(instr) => {
                    let end = pc + instr.word_len().saturating_sub(1);
                    high = high.max(end);
                    pc += instr.word_len();
                }
                AsmLine::Label(_) | AsmLine::Comment(_) => {}
            }
        }
        high
    }

    /// Emits one runtime helper body that obeys the repaired Phase 4 stack-first ABI.
    fn emit_runtime_helper(&mut self, helper: RuntimeHelper) {
        let info = helper.info();
        let ty = info.operand_ty;
        let width = ty.byte_width() as u16;
        let arg0_offset = 0u16;
        let arg1_offset = width;
        let local_base = info.arg_bytes + 2;
        let work_offset = local_base;
        let count_offset = work_offset + width;
        let flag_offset = count_offset + 1;
        let catalog = helper.catalog_entry();

        self.program.push(AsmLine::Label(info.label.to_string()));
        self.program.push(AsmLine::Comment(format!(
            "runtime helper: {} helper={} variant={} runtime_profile={} math_profile={} required_by={} args={} locals={} frame_bytes={}",
            catalog.category.as_str(),
            info.label,
            runtime_helper_variant(helper, self.options.runtime_profile, self.options.math_profile),
            self.options.runtime_profile.as_str(),
            self.options.math_profile.as_str(),
            catalog.required_by,
            info.arg_bytes,
            info.local_bytes,
            info.frame_bytes
        )));
        self.current_bank = UNKNOWN_BANK;
        self.emit_runtime_prologue(info);

        match helper {
            RuntimeHelper::F32Cmp => {
                self.emit_float_f32_compare_helper(local_base);
            }
            RuntimeHelper::F32Fabs => {
                self.emit_float_f32_fabs_helper(arg0_offset);
            }
            RuntimeHelper::F32Trunc => {
                self.emit_float_f32_trunc_helper(arg0_offset, work_offset);
            }
            RuntimeHelper::F32Floor => {
                self.emit_float_f32_floor_helper(arg0_offset, local_base);
            }
            RuntimeHelper::F32Ceil => {
                self.emit_float_f32_ceil_helper(arg0_offset, local_base);
            }
            RuntimeHelper::F32Round => {
                self.emit_float_f32_round_helper(arg0_offset, local_base);
            }
            RuntimeHelper::F32Sqrt => {
                if self.options.math_profile == MathProfile::Precise {
                    self.emit_float_f32_sqrt_precise_helper(arg0_offset, local_base);
                } else {
                    self.emit_float_f32_sqrt_helper(arg0_offset, local_base);
                }
            }
            RuntimeHelper::F32ToQ16 => {
                self.emit_float_to_q16_frame(
                    0,
                    local_base,
                    local_base + 4,
                    local_base + 8,
                    local_base + 9,
                    local_base + 10,
                    local_base + 11,
                );
                self.emit_return_current_frame_value(local_base, Type::new(ScalarType::Q16_16));
            }
            RuntimeHelper::Q16ToF32 => {
                self.emit_q16_to_float_frame(
                    0,
                    local_base,
                    local_base + 4,
                    local_base + 8,
                    local_base + 9,
                    local_base + 10,
                );
                self.emit_return_current_frame_value(local_base, Type::new(ScalarType::F32));
            }
            RuntimeHelper::I32ToF32 | RuntimeHelper::U32ToF32 => {
                self.emit_i32_to_float_frame(
                    0,
                    local_base,
                    local_base + 4,
                    local_base + 8,
                    local_base + 9,
                    local_base + 10,
                    matches!(helper, RuntimeHelper::I32ToF32),
                );
                self.emit_return_current_frame_value(local_base, Type::new(ScalarType::F32));
            }
            RuntimeHelper::F32ToI32 | RuntimeHelper::F32ToU32 => {
                let result_ty = if matches!(helper, RuntimeHelper::F32ToU32) {
                    Type::new(ScalarType::U32)
                } else {
                    Type::new(ScalarType::I32)
                };
                self.emit_float_to_i32_frame(
                    0,
                    local_base,
                    local_base + 4,
                    local_base + 5,
                    local_base + 6,
                    local_base + 7,
                    matches!(helper, RuntimeHelper::F32ToI32),
                );
                self.emit_return_current_frame_value(local_base, result_ty);
            }
            RuntimeHelper::F32Sub if self.options.runtime_profile == RuntimeProfile::Small => {
                self.emit_f32_sub_small_wrapper(arg0_offset, arg1_offset, work_offset);
            }
            RuntimeHelper::F32Add
            | RuntimeHelper::F32Sub
            | RuntimeHelper::F32Mul
            | RuntimeHelper::F32Div => {
                self.emit_float_f32_binary_helper(helper, local_base);
            }
            RuntimeHelper::MulQ8_8 | RuntimeHelper::MulUQ8_8 => {
                self.emit_fixed_q8_8_mul_helper(
                    ty,
                    local_base,
                    matches!(helper, RuntimeHelper::MulQ8_8),
                );
            }
            RuntimeHelper::MulQ16_16 if self.options.runtime_profile == RuntimeProfile::Small => {
                self.emit_q16_16_small_signed_wrapper(
                    RuntimeHelper::MulUQ16_16,
                    arg0_offset,
                    arg1_offset,
                    work_offset,
                    flag_offset,
                    Type::new(ScalarType::Q16_16),
                );
            }
            RuntimeHelper::MulQ16_16 | RuntimeHelper::MulUQ16_16 => {
                self.emit_fixed_q16_16_mul_helper(
                    ty,
                    local_base,
                    matches!(helper, RuntimeHelper::MulQ16_16),
                );
            }
            RuntimeHelper::MulU8 | RuntimeHelper::MulU16 | RuntimeHelper::MulU32 => {
                self.clear_current_frame_slot(work_offset, ty);
                self.emit_const_to_w(ty.bit_width() as u8);
                self.store_w_to_current_frame_byte(count_offset);
                self.emit_unsigned_mul_core(
                    arg0_offset,
                    arg1_offset,
                    work_offset,
                    count_offset,
                    ty,
                );
                self.emit_return_current_frame_value(work_offset, ty);
            }
            RuntimeHelper::MulI8 | RuntimeHelper::MulI16 | RuntimeHelper::MulI32 => {
                self.clear_current_frame_slot(flag_offset, Type::new(ScalarType::U8));
                self.emit_runtime_negate_if_signed(arg0_offset, ty, flag_offset, 0x01);
                self.emit_runtime_negate_if_signed(arg1_offset, ty, flag_offset, 0x01);
                self.clear_current_frame_slot(work_offset, ty);
                self.emit_const_to_w(ty.bit_width() as u8);
                self.store_w_to_current_frame_byte(count_offset);
                self.emit_unsigned_mul_core(
                    arg0_offset,
                    arg1_offset,
                    work_offset,
                    count_offset,
                    ty,
                );

                let negate_label = self.unique_label("rt_mul_neg");
                let done_label = self.unique_label("rt_mul_done");
                self.branch_on_current_frame_bit(flag_offset, 0, &negate_label, &done_label);
                self.program.push(AsmLine::Label(negate_label));
                self.negate_current_frame_value(work_offset, ty);
                self.program.push(AsmLine::Label(done_label));
                self.emit_return_current_frame_value(work_offset, ty);
            }
            RuntimeHelper::DivQ8_8 | RuntimeHelper::DivUQ8_8 => {
                self.emit_fixed_q8_8_div_helper(
                    ty,
                    local_base,
                    matches!(helper, RuntimeHelper::DivQ8_8),
                );
            }
            RuntimeHelper::DivQ16_16 if self.options.runtime_profile == RuntimeProfile::Small => {
                self.emit_q16_16_small_signed_wrapper(
                    RuntimeHelper::DivUQ16_16,
                    arg0_offset,
                    arg1_offset,
                    work_offset,
                    flag_offset,
                    Type::new(ScalarType::Q16_16),
                );
            }
            RuntimeHelper::DivQ16_16 | RuntimeHelper::DivUQ16_16 => {
                self.emit_fixed_q16_16_div_helper(
                    ty,
                    local_base,
                    matches!(helper, RuntimeHelper::DivQ16_16),
                );
            }
            RuntimeHelper::U32DivModCore => {
                self.emit_u32_divmod_core_helper(local_base);
            }
            RuntimeHelper::DivU32 | RuntimeHelper::ModU32
                if self.options.runtime_profile == RuntimeProfile::Small =>
            {
                self.emit_u32_divmod_small_wrapper(
                    helper,
                    arg0_offset,
                    arg1_offset,
                    work_offset,
                    flag_offset,
                    false,
                );
            }
            RuntimeHelper::DivI32 | RuntimeHelper::ModI32
                if self.options.runtime_profile == RuntimeProfile::Small =>
            {
                self.emit_u32_divmod_small_wrapper(
                    helper,
                    arg0_offset,
                    arg1_offset,
                    work_offset,
                    flag_offset,
                    true,
                );
            }
            RuntimeHelper::DivU8
            | RuntimeHelper::DivU16
            | RuntimeHelper::DivU32
            | RuntimeHelper::ModU8
            | RuntimeHelper::ModU16
            | RuntimeHelper::ModU32 => {
                let core_label = self.unique_label("rt_udiv_core");
                let zero_label = self.unique_label("rt_udiv_zero");
                let finish_label = self.unique_label("rt_udiv_finish");
                self.clear_current_frame_slot(work_offset, ty);
                self.emit_const_to_w(ty.bit_width() as u8);
                self.store_w_to_current_frame_byte(count_offset);
                self.emit_current_frame_nonzero_branch(arg1_offset, ty, &core_label, &zero_label);
                self.program.push(AsmLine::Label(core_label));
                self.emit_unsigned_divmod_core(
                    arg0_offset,
                    arg1_offset,
                    work_offset,
                    count_offset,
                    ty,
                    &finish_label,
                );
                self.program.push(AsmLine::Label(zero_label));
                self.clear_current_frame_slot(arg0_offset, ty);
                self.clear_current_frame_slot(work_offset, ty);
                self.jump_to_label(&finish_label);
                self.program.push(AsmLine::Label(finish_label));
                let result_offset = if matches!(
                    helper,
                    RuntimeHelper::DivU8 | RuntimeHelper::DivU16 | RuntimeHelper::DivU32
                ) {
                    arg0_offset
                } else {
                    work_offset
                };
                self.emit_return_current_frame_value(result_offset, ty);
            }
            RuntimeHelper::DivI8
            | RuntimeHelper::DivI16
            | RuntimeHelper::DivI32
            | RuntimeHelper::ModI8
            | RuntimeHelper::ModI16
            | RuntimeHelper::ModI32 => {
                let core_label = self.unique_label("rt_sdiv_core");
                let zero_label = self.unique_label("rt_sdiv_zero");
                let finish_label = self.unique_label("rt_sdiv_finish");
                self.clear_current_frame_slot(flag_offset, Type::new(ScalarType::U8));
                self.emit_runtime_set_flag_if_signed(arg0_offset, ty, flag_offset, 0x02);
                self.emit_runtime_negate_if_signed(arg0_offset, ty, flag_offset, 0x01);
                self.emit_runtime_negate_if_signed(arg1_offset, ty, flag_offset, 0x01);
                self.clear_current_frame_slot(work_offset, ty);
                self.emit_const_to_w(ty.bit_width() as u8);
                self.store_w_to_current_frame_byte(count_offset);
                self.emit_current_frame_nonzero_branch(arg1_offset, ty, &core_label, &zero_label);
                self.program.push(AsmLine::Label(core_label));
                self.emit_unsigned_divmod_core(
                    arg0_offset,
                    arg1_offset,
                    work_offset,
                    count_offset,
                    ty,
                    &finish_label,
                );
                self.program.push(AsmLine::Label(zero_label));
                self.clear_current_frame_slot(arg0_offset, ty);
                self.clear_current_frame_slot(work_offset, ty);
                self.jump_to_label(&finish_label);
                self.program.push(AsmLine::Label(finish_label));

                if matches!(
                    helper,
                    RuntimeHelper::DivI8 | RuntimeHelper::DivI16 | RuntimeHelper::DivI32
                ) {
                    let negate_label = self.unique_label("rt_sdiv_neg");
                    let done_label = self.unique_label("rt_sdiv_done");
                    self.branch_on_current_frame_bit(flag_offset, 0, &negate_label, &done_label);
                    self.program.push(AsmLine::Label(negate_label));
                    self.negate_current_frame_value(arg0_offset, ty);
                    self.program.push(AsmLine::Label(done_label));
                    self.emit_return_current_frame_value(arg0_offset, ty);
                } else {
                    let negate_label = self.unique_label("rt_smod_neg");
                    let done_label = self.unique_label("rt_smod_done");
                    self.branch_on_current_frame_bit(flag_offset, 1, &negate_label, &done_label);
                    self.program.push(AsmLine::Label(negate_label));
                    self.negate_current_frame_value(work_offset, ty);
                    self.program.push(AsmLine::Label(done_label));
                    self.emit_return_current_frame_value(work_offset, ty);
                }
            }
            RuntimeHelper::Shl8
            | RuntimeHelper::Shl16
            | RuntimeHelper::Shl32
            | RuntimeHelper::ShrU8
            | RuntimeHelper::ShrI8
            | RuntimeHelper::ShrU16
            | RuntimeHelper::ShrI16
            | RuntimeHelper::ShrU32
            | RuntimeHelper::ShrI32 => {
                let loop_label = self.unique_label("rt_shift_loop");
                let body_label = self.unique_label("rt_shift_body");
                let done_label = self.unique_label("rt_shift_done");
                self.clamp_runtime_shift_count(arg1_offset, ty);
                self.program.push(AsmLine::Label(loop_label.clone()));
                self.emit_current_frame_nonzero_branch(arg1_offset, ty, &body_label, &done_label);
                self.program.push(AsmLine::Label(body_label));
                if matches!(
                    helper,
                    RuntimeHelper::Shl8 | RuntimeHelper::Shl16 | RuntimeHelper::Shl32
                ) {
                    self.shift_current_frame_value_left(arg0_offset, ty);
                } else {
                    self.shift_current_frame_value_right(
                        arg0_offset,
                        ty,
                        matches!(
                            helper,
                            RuntimeHelper::ShrI8 | RuntimeHelper::ShrI16 | RuntimeHelper::ShrI32
                        ),
                    );
                }
                self.decrement_current_frame_value(arg1_offset, ty);
                self.jump_to_label(&loop_label);
                self.program.push(AsmLine::Label(done_label));
                self.emit_return_current_frame_value(arg0_offset, ty);
            }
        }

        self.emit_runtime_epilogue(info);
        self.program.push(AsmLine::Instr(AsmInstr::Return));
    }

    /// Emits Phase 27 finite f32 arithmetic through an internal Q16.16 work format.
    fn emit_float_f32_binary_helper(&mut self, helper: RuntimeHelper, local_base: u16) {
        let f32_ty = Type::new(ScalarType::F32);
        let q_ty = Type::new(ScalarType::I32);
        let q0_offset = local_base;
        let q1_offset = q0_offset + 4;
        let mant_offset = q1_offset + 4;
        let exp_offset = mant_offset + 4;
        let count_offset = exp_offset + 1;
        let const_offset = count_offset + 1;
        let flag_offset = const_offset + 1;
        let main_label = self.unique_label("rt_f32_main");
        let convert_label = self.unique_label("rt_f32_to_q16_call");

        self.jump_to_label(&main_label);
        self.program.push(AsmLine::Label(convert_label.clone()));
        self.emit_float_to_q16_frame(
            0,
            q1_offset,
            mant_offset,
            exp_offset,
            count_offset,
            const_offset,
            flag_offset,
        );
        self.program.push(AsmLine::Instr(AsmInstr::Return));

        self.program.push(AsmLine::Label(main_label));
        self.program
            .push(AsmLine::Instr(AsmInstr::SetPage(convert_label.clone())));
        self.program
            .push(AsmLine::Instr(AsmInstr::Call(convert_label.clone())));
        self.restore_code_page_after_call();
        self.copy_current_frame_bytes(q1_offset, q0_offset, 4);
        self.copy_current_frame_bytes(4, 0, 4);
        self.program
            .push(AsmLine::Instr(AsmInstr::SetPage(convert_label.clone())));
        self.program
            .push(AsmLine::Instr(AsmInstr::Call(convert_label)));
        self.restore_code_page_after_call();

        match helper {
            RuntimeHelper::F32Add => {
                self.add_current_frame_value_into_slot(q1_offset, q0_offset, q_ty);
            }
            RuntimeHelper::F32Sub => {
                self.sub_current_frame_value_from_slot(q1_offset, q0_offset, q_ty);
            }
            RuntimeHelper::F32Mul => {
                self.copy_current_frame_bytes(q0_offset, 0, 4);
                self.copy_current_frame_bytes(q1_offset, 4, 4);
                self.emit_fixed_q16_16_mul_helper(Type::new(ScalarType::Q16_16), local_base, true);
                self.store_w_to_current_frame_byte(q0_offset);
                for byte in 1..4 {
                    self.load_return_byte_to_w(byte);
                    self.store_w_to_current_frame_byte(q0_offset + byte as u16);
                }
            }
            RuntimeHelper::F32Div => {
                self.copy_current_frame_bytes(q0_offset, 0, 4);
                self.copy_current_frame_bytes(q1_offset, 4, 4);
                self.emit_fixed_q16_16_div_helper(Type::new(ScalarType::Q16_16), local_base, true);
                self.store_w_to_current_frame_byte(q0_offset);
                for byte in 1..4 {
                    self.load_return_byte_to_w(byte);
                    self.store_w_to_current_frame_byte(q0_offset + byte as u16);
                }
            }
            _ => unreachable!("float helper"),
        }

        self.emit_q16_to_float_frame(
            q0_offset,
            0,
            mant_offset,
            exp_offset,
            count_offset,
            flag_offset,
        );
        self.emit_return_current_frame_value(0, f32_ty);
    }

    /// Compares two finite f32 values by converting them to signed Q16.16 work values.
    fn emit_float_f32_compare_helper(&mut self, local_base: u16) {
        let q_ty = Type::new(ScalarType::I32);
        let q0_offset = local_base;
        let q1_offset = q0_offset + 4;
        let exp_offset = q1_offset + 4;
        let count_offset = exp_offset + 1;
        let const_offset = count_offset + 1;
        let flag_offset = const_offset + 1;
        let result_offset = flag_offset + 1;
        let equal_label = self.unique_label("rt_f32_cmp_equal");
        let not_equal_label = self.unique_label("rt_f32_cmp_not_equal");
        let lhs_neg_label = self.unique_label("rt_f32_cmp_lhs_neg");
        let lhs_nonneg_label = self.unique_label("rt_f32_cmp_lhs_nonneg");
        let rhs_neg_from_lhs_neg_label = self.unique_label("rt_f32_cmp_rhs_neg_lhs_neg");
        let rhs_neg_from_lhs_nonneg_label = self.unique_label("rt_f32_cmp_rhs_neg_lhs_nonneg");
        let same_sign_label = self.unique_label("rt_f32_cmp_same_sign");
        let less_label = self.unique_label("rt_f32_cmp_less");
        let greater_label = self.unique_label("rt_f32_cmp_greater");
        let finish_label = self.unique_label("rt_f32_cmp_finish");

        self.emit_float_to_q16_frame(
            0,
            q0_offset,
            q1_offset,
            exp_offset,
            count_offset,
            const_offset,
            flag_offset,
        );
        self.emit_float_to_q16_frame(
            4,
            q1_offset,
            q0_offset,
            exp_offset,
            count_offset,
            const_offset,
            flag_offset,
        );

        self.emit_current_frame_equal_branch(
            q0_offset,
            q1_offset,
            q_ty,
            &equal_label,
            &not_equal_label,
        );

        self.program.push(AsmLine::Label(equal_label));
        self.emit_const_to_w(0);
        self.store_w_to_current_frame_byte(result_offset);
        self.jump_to_label(&finish_label);

        self.program.push(AsmLine::Label(not_equal_label));
        self.branch_on_current_frame_bit(q0_offset + 3, 7, &lhs_neg_label, &lhs_nonneg_label);

        self.program.push(AsmLine::Label(lhs_neg_label));
        self.branch_on_current_frame_bit(
            q1_offset + 3,
            7,
            &rhs_neg_from_lhs_neg_label,
            &less_label,
        );
        self.program
            .push(AsmLine::Label(rhs_neg_from_lhs_neg_label));
        self.jump_to_label(&same_sign_label);

        self.program.push(AsmLine::Label(lhs_nonneg_label));
        self.branch_on_current_frame_bit(
            q1_offset + 3,
            7,
            &rhs_neg_from_lhs_nonneg_label,
            &same_sign_label,
        );
        self.program
            .push(AsmLine::Label(rhs_neg_from_lhs_nonneg_label));
        self.jump_to_label(&greater_label);

        self.program.push(AsmLine::Label(same_sign_label));
        self.emit_current_frame_unsigned_ge_branch(
            q0_offset,
            q1_offset,
            q_ty,
            &greater_label,
            &less_label,
        );

        self.program.push(AsmLine::Label(less_label));
        self.emit_const_to_w(0xFF);
        self.store_w_to_current_frame_byte(result_offset);
        self.jump_to_label(&finish_label);

        self.program.push(AsmLine::Label(greater_label));
        self.emit_const_to_w(1);
        self.store_w_to_current_frame_byte(result_offset);

        self.program.push(AsmLine::Label(finish_label));
        self.load_current_frame_byte_to_w(result_offset);
    }

    /// Emits `fabsf` by clearing the finite f32 sign bit.
    fn emit_float_f32_fabs_helper(&mut self, arg_offset: u16) {
        self.load_current_frame_byte_to_w(arg_offset + 3);
        self.program.push(AsmLine::Instr(AsmInstr::Andlw(0x7F)));
        self.store_w_to_current_frame_byte(arg_offset + 3);
        self.emit_return_current_frame_value(arg_offset, Type::new(ScalarType::F32));
    }

    /// Emits `truncf(x)` through the existing finite f32 <-> i32 conversion policy.
    fn emit_float_f32_trunc_helper(&mut self, arg_offset: u16, result_offset: u16) {
        self.emit_call_unary_runtime_helper_32(RuntimeHelper::F32ToI32, arg_offset, result_offset);
        self.emit_call_unary_runtime_helper_32(
            RuntimeHelper::I32ToF32,
            result_offset,
            result_offset,
        );
        self.emit_return_current_frame_value(result_offset, Type::new(ScalarType::F32));
    }

    /// Emits `floorf(x)` by converting to Q16.16 and shifting arithmetically.
    fn emit_float_f32_floor_helper(&mut self, arg_offset: u16, local_base: u16) {
        let q_offset = local_base;

        self.emit_call_unary_runtime_helper_32(RuntimeHelper::F32ToQ16, arg_offset, q_offset);
        for _ in 0..16 {
            self.shift_current_frame_value_right(q_offset, Type::new(ScalarType::I32), true);
        }
        self.emit_call_unary_runtime_helper_32(RuntimeHelper::I32ToF32, q_offset, q_offset);
        self.emit_return_current_frame_value(q_offset, Type::new(ScalarType::F32));
    }

    /// Emits `ceilf(x)` as `-floorf(-x)` so it reuses the validated floor helper.
    fn emit_float_f32_ceil_helper(&mut self, arg_offset: u16, local_base: u16) {
        let work_offset = local_base;
        self.copy_current_frame_bytes(arg_offset, work_offset, 4);
        self.load_current_frame_byte_to_w(work_offset + 3);
        self.program.push(AsmLine::Instr(AsmInstr::Xorlw(0x80)));
        self.store_w_to_current_frame_byte(work_offset + 3);
        self.emit_call_unary_runtime_helper_32(RuntimeHelper::F32Floor, work_offset, work_offset);
        self.load_current_frame_byte_to_w(work_offset + 3);
        self.program.push(AsmLine::Instr(AsmInstr::Xorlw(0x80)));
        self.store_w_to_current_frame_byte(work_offset + 3);
        self.emit_return_current_frame_value(work_offset, Type::new(ScalarType::F32));
    }

    /// Emits `roundf` with Phase 36 half-away-from-zero behavior.
    fn emit_float_f32_round_helper(&mut self, arg_offset: u16, local_base: u16) {
        let q_offset = local_base;
        let half_offset = q_offset + 4;
        let flag_offset = half_offset + 4;
        let negative_label = self.unique_label("rt_f32_round_negative");
        let abs_done_label = self.unique_label("rt_f32_round_abs_done");
        let apply_sign_label = self.unique_label("rt_f32_round_apply_sign");
        let done_label = self.unique_label("rt_f32_round_done");

        self.emit_call_unary_runtime_helper_32(RuntimeHelper::F32ToQ16, arg_offset, q_offset);
        self.clear_current_frame_slot(flag_offset, Type::new(ScalarType::U8));
        self.branch_on_current_frame_bit(q_offset + 3, 7, &negative_label, &abs_done_label);
        self.program.push(AsmLine::Label(negative_label));
        self.set_current_frame_bit(flag_offset, 0);
        self.negate_current_frame_value(q_offset, Type::new(ScalarType::I32));

        self.program.push(AsmLine::Label(abs_done_label));
        self.store_i32_const_to_current_frame(half_offset, 0x8000);
        self.add_current_frame_value_into_slot(half_offset, q_offset, Type::new(ScalarType::I32));
        for _ in 0..16 {
            self.shift_current_frame_value_right(q_offset, Type::new(ScalarType::I32), false);
        }
        self.branch_on_current_frame_bit(flag_offset, 0, &apply_sign_label, &done_label);
        self.program.push(AsmLine::Label(apply_sign_label));
        self.negate_current_frame_value(q_offset, Type::new(ScalarType::I32));
        self.program.push(AsmLine::Label(done_label));
        self.emit_call_unary_runtime_helper_32(RuntimeHelper::I32ToF32, q_offset, q_offset);
        self.emit_return_current_frame_value(q_offset, Type::new(ScalarType::F32));
    }

    /// Emits compact finite `sqrtf` with exact validated cases and a bit-level fallback.
    fn emit_float_f32_sqrt_helper(&mut self, arg_offset: u16, local_base: u16) {
        let f32_ty = Type::new(ScalarType::F32);
        let u32_ty = Type::new(ScalarType::U32);
        let result_offset = local_base;
        let const_offset = result_offset + 4;
        let zero_label = self.unique_label("rt_f32_sqrt_zero");
        let positive_label = self.unique_label("rt_f32_sqrt_positive");
        let nonnegative_label = self.unique_label("rt_f32_sqrt_nonnegative");
        let fallback_label = self.unique_label("rt_f32_sqrt_fallback");
        let finish_label = self.unique_label("rt_f32_sqrt_finish");

        self.emit_current_frame_nonzero_branch(arg_offset, f32_ty, &positive_label, &zero_label);
        self.program.push(AsmLine::Label(positive_label));
        self.branch_on_current_frame_bit(arg_offset + 3, 7, &zero_label, &nonnegative_label);

        self.program.push(AsmLine::Label(nonnegative_label));
        self.emit_f32_sqrt_const_case(
            arg_offset,
            const_offset,
            result_offset,
            0x3F80_0000,
            0x3F80_0000,
            &finish_label,
        );
        self.emit_f32_sqrt_const_case(
            arg_offset,
            const_offset,
            result_offset,
            0x4080_0000,
            0x4000_0000,
            &finish_label,
        );
        self.emit_f32_sqrt_const_case(
            arg_offset,
            const_offset,
            result_offset,
            0x4110_0000,
            0x4040_0000,
            &finish_label,
        );
        self.emit_f32_sqrt_const_case(
            arg_offset,
            const_offset,
            result_offset,
            0x4010_0000,
            0x3FC0_0000,
            &finish_label,
        );
        self.emit_f32_sqrt_const_case(
            arg_offset,
            const_offset,
            result_offset,
            0x3E80_0000,
            0x3F00_0000,
            &finish_label,
        );

        self.program.push(AsmLine::Label(fallback_label));
        self.copy_current_frame_bytes(arg_offset, result_offset, 4);
        self.shift_current_frame_value_right(result_offset, u32_ty, false);
        self.store_i32_const_to_current_frame(const_offset, 0x1FC0_0000);
        self.add_current_frame_value_into_slot(const_offset, result_offset, u32_ty);
        self.jump_to_label(&finish_label);

        self.program.push(AsmLine::Label(zero_label));
        self.clear_current_frame_slot(result_offset, f32_ty);
        self.jump_to_label(&finish_label);

        self.program.push(AsmLine::Label(finish_label));
        self.emit_return_current_frame_value(result_offset, f32_ty);
    }

    /// Emits finite `sqrtf` for `--math-profile precise` with a refined validated-value table.
    fn emit_float_f32_sqrt_precise_helper(&mut self, arg_offset: u16, local_base: u16) {
        let f32_ty = Type::new(ScalarType::F32);
        let u32_ty = Type::new(ScalarType::U32);
        let result_offset = local_base;
        let const_offset = result_offset + 4;
        let zero_label = self.unique_label("rt_f32_sqrt_precise_zero");
        let positive_label = self.unique_label("rt_f32_sqrt_precise_positive");
        let nonnegative_label = self.unique_label("rt_f32_sqrt_precise_nonnegative");
        let fallback_label = self.unique_label("rt_f32_sqrt_precise_fallback");
        let finish_label = self.unique_label("rt_f32_sqrt_precise_finish");

        self.emit_current_frame_nonzero_branch(arg_offset, f32_ty, &positive_label, &zero_label);
        self.program.push(AsmLine::Label(positive_label));
        self.branch_on_current_frame_bit(arg_offset + 3, 7, &zero_label, &nonnegative_label);

        self.program.push(AsmLine::Label(nonnegative_label));
        self.emit_f32_sqrt_const_case(
            arg_offset,
            const_offset,
            result_offset,
            0x3F80_0000,
            0x3F80_0000,
            &finish_label,
        );
        self.emit_f32_sqrt_const_case(
            arg_offset,
            const_offset,
            result_offset,
            0x4080_0000,
            0x4000_0000,
            &finish_label,
        );
        self.emit_f32_sqrt_const_case(
            arg_offset,
            const_offset,
            result_offset,
            0x4110_0000,
            0x4040_0000,
            &finish_label,
        );
        self.emit_f32_sqrt_const_case(
            arg_offset,
            const_offset,
            result_offset,
            0x4010_0000,
            0x3FC0_0000,
            &finish_label,
        );
        self.emit_f32_sqrt_const_case(
            arg_offset,
            const_offset,
            result_offset,
            0x3E80_0000,
            0x3F00_0000,
            &finish_label,
        );
        self.emit_f32_sqrt_const_case(
            arg_offset,
            const_offset,
            result_offset,
            0x4000_0000,
            0x3FB5_04F3,
            &finish_label,
        );
        self.emit_f32_sqrt_const_case(
            arg_offset,
            const_offset,
            result_offset,
            0x4040_0000,
            0x3FDD_B3D7,
            &finish_label,
        );
        self.emit_f32_sqrt_const_case(
            arg_offset,
            const_offset,
            result_offset,
            0x4120_0000,
            0x404A_62C2,
            &finish_label,
        );
        self.emit_f32_sqrt_const_case(
            arg_offset,
            const_offset,
            result_offset,
            0x3F00_0000,
            0x3F35_04F3,
            &finish_label,
        );

        self.program.push(AsmLine::Label(fallback_label));
        self.copy_current_frame_bytes(arg_offset, result_offset, 4);
        self.shift_current_frame_value_right(result_offset, u32_ty, false);
        self.store_i32_const_to_current_frame(const_offset, 0x1FC0_0000);
        self.add_current_frame_value_into_slot(const_offset, result_offset, u32_ty);
        self.jump_to_label(&finish_label);

        self.program.push(AsmLine::Label(zero_label));
        self.clear_current_frame_slot(result_offset, f32_ty);
        self.jump_to_label(&finish_label);

        self.program.push(AsmLine::Label(finish_label));
        self.emit_return_current_frame_value(result_offset, f32_ty);
    }

    fn emit_f32_sqrt_const_case(
        &mut self,
        arg_offset: u16,
        const_offset: u16,
        result_offset: u16,
        input_bits: i64,
        result_bits: i64,
        finish_label: &str,
    ) {
        let hit_label = self.unique_label("rt_f32_sqrt_const");
        let miss_label = self.unique_label("rt_f32_sqrt_next_const");
        self.store_i32_const_to_current_frame(const_offset, input_bits);
        self.emit_current_frame_equal_branch(
            arg_offset,
            const_offset,
            Type::new(ScalarType::U32),
            &hit_label,
            &miss_label,
        );
        self.program.push(AsmLine::Label(hit_label));
        self.store_i32_const_to_current_frame(result_offset, result_bits);
        self.jump_to_label(finish_label);
        self.program.push(AsmLine::Label(miss_label));
    }

    /// Emits compact Phase 35 f32 subtraction as `lhs + (-rhs)` through `__rt_f32_add`.
    fn emit_f32_sub_small_wrapper(&mut self, lhs_offset: u16, rhs_offset: u16, result_offset: u16) {
        self.load_current_frame_byte_to_w(rhs_offset + 3);
        self.program.push(AsmLine::Instr(AsmInstr::Xorlw(0x80)));
        self.store_w_to_current_frame_byte(rhs_offset + 3);
        self.emit_call_binary_runtime_helper_32(
            RuntimeHelper::F32Add,
            lhs_offset,
            rhs_offset,
            result_offset,
        );
        self.emit_return_current_frame_value(result_offset, Type::new(ScalarType::F32));
    }

    /// Converts a signed or unsigned 32-bit integer to finite IEEE f32 bits.
    #[allow(clippy::too_many_arguments)]
    fn emit_i32_to_float_frame(
        &mut self,
        int_offset: u16,
        raw_offset: u16,
        mant_offset: u16,
        exp_offset: u16,
        count_offset: u16,
        flag_offset: u16,
        signed: bool,
    ) {
        self.emit_int_to_float_frame(
            int_offset,
            raw_offset,
            mant_offset,
            exp_offset,
            count_offset,
            flag_offset,
            signed,
            150,
        );
    }

    /// Converts finite IEEE f32 bits to signed or unsigned 32-bit integer bits.
    #[allow(clippy::too_many_arguments)]
    fn emit_float_to_i32_frame(
        &mut self,
        raw_offset: u16,
        int_offset: u16,
        exp_offset: u16,
        count_offset: u16,
        const_offset: u16,
        flag_offset: u16,
        signed: bool,
    ) {
        self.emit_float_to_scaled_int_frame(
            raw_offset,
            int_offset,
            exp_offset,
            count_offset,
            const_offset,
            flag_offset,
            signed,
            150,
        );
    }

    /// Converts one active-frame signed/unsigned integer slot to raw finite f32 bits.
    #[allow(clippy::too_many_arguments)]
    fn emit_int_to_float_frame(
        &mut self,
        int_offset: u16,
        raw_offset: u16,
        mant_offset: u16,
        exp_offset: u16,
        count_offset: u16,
        flag_offset: u16,
        signed: bool,
        base_exp: u8,
    ) {
        let i32_ty = Type::new(ScalarType::I32);
        let u8_ty = Type::new(ScalarType::U8);
        let nonzero_label = self.unique_label("rt_i32_to_f32_nonzero");
        let done_label = self.unique_label("rt_i32_to_f32_done");
        let sign_done_label = self.unique_label("rt_i32_to_f32_sign_done");
        let pack_label = self.unique_label("rt_i32_to_f32_pack");
        let high_loop_label = self.unique_label("rt_i32_to_f32_high_loop");
        let high_body_label = self.unique_label("rt_i32_to_f32_high_body");
        let low_loop_label = self.unique_label("rt_i32_to_f32_low_loop");
        let low_body_label = self.unique_label("rt_i32_to_f32_low_body");

        self.clear_current_frame_slot(raw_offset, Type::new(ScalarType::F32));
        self.clear_current_frame_slot(flag_offset, u8_ty);
        self.emit_current_frame_nonzero_branch(int_offset, i32_ty, &nonzero_label, &done_label);
        self.program.push(AsmLine::Label(nonzero_label));
        if signed {
            let sign_label = self.unique_label("rt_i32_to_f32_sign");
            self.branch_on_current_frame_bit(int_offset + 3, 7, &sign_label, &sign_done_label);
            self.program.push(AsmLine::Label(sign_label));
            self.set_current_frame_bit(flag_offset, 0);
            self.negate_current_frame_value(int_offset, i32_ty);
        }
        self.program.push(AsmLine::Label(sign_done_label));

        self.copy_current_frame_bytes(int_offset, mant_offset, 4);
        self.emit_const_to_w(base_exp);
        self.store_w_to_current_frame_byte(exp_offset);

        self.program.push(AsmLine::Label(high_loop_label.clone()));
        self.emit_current_frame_nonzero_branch(
            mant_offset + 3,
            u8_ty,
            &high_body_label,
            &low_loop_label,
        );
        self.program.push(AsmLine::Label(high_body_label));
        self.shift_current_frame_value_right(mant_offset, i32_ty, false);
        self.emit_const_to_w(1);
        self.store_w_to_current_frame_byte(count_offset);
        self.add_current_frame_value_into_slot(count_offset, exp_offset, u8_ty);
        self.jump_to_label(&high_loop_label);

        self.program.push(AsmLine::Label(low_loop_label.clone()));
        self.branch_on_current_frame_bit(mant_offset + 2, 7, &pack_label, &low_body_label);
        self.program.push(AsmLine::Label(low_body_label));
        self.shift_current_frame_value_left(mant_offset, i32_ty);
        self.decrement_current_frame_value(exp_offset, u8_ty);
        self.jump_to_label(&low_loop_label);

        self.program.push(AsmLine::Label(pack_label));
        self.copy_current_frame_bytes(mant_offset, raw_offset, 2);
        self.load_current_frame_byte_to_w(mant_offset + 2);
        self.program.push(AsmLine::Instr(AsmInstr::Andlw(0x7F)));
        self.store_w_to_current_frame_byte(raw_offset + 2);
        let exp_low_label = self.unique_label("rt_i32_to_f32_exp_low");
        let exp_done_label = self.unique_label("rt_i32_to_f32_exp_done");
        self.branch_on_current_frame_bit(exp_offset, 0, &exp_low_label, &exp_done_label);
        self.program.push(AsmLine::Label(exp_low_label));
        self.set_current_frame_bit(raw_offset + 2, 7);
        self.program.push(AsmLine::Label(exp_done_label));
        self.copy_current_frame_bytes(exp_offset, count_offset, 1);
        self.shift_current_frame_value_right(count_offset, u8_ty, false);
        self.load_current_frame_byte_to_w(count_offset);
        self.store_w_to_current_frame_byte(raw_offset + 3);
        let sign_set_label = self.unique_label("rt_i32_to_f32_sign_set");
        let sign_finish_label = self.unique_label("rt_i32_to_f32_sign_finish");
        self.branch_on_current_frame_bit(flag_offset, 0, &sign_set_label, &sign_finish_label);
        self.program.push(AsmLine::Label(sign_set_label));
        self.set_current_frame_bit(raw_offset + 3, 7);
        self.program.push(AsmLine::Label(sign_finish_label));
        self.program.push(AsmLine::Label(done_label));
    }

    /// Converts raw finite f32 bits to a scaled signed/unsigned integer slot.
    #[allow(clippy::too_many_arguments)]
    fn emit_float_to_scaled_int_frame(
        &mut self,
        raw_offset: u16,
        int_offset: u16,
        exp_offset: u16,
        count_offset: u16,
        const_offset: u16,
        flag_offset: u16,
        signed: bool,
        base_exp: u8,
    ) {
        let i32_ty = Type::new(ScalarType::I32);
        let u8_ty = Type::new(ScalarType::U8);
        let nonzero_label = self.unique_label("rt_f32_to_i32_nonzero");
        let done_label = self.unique_label("rt_f32_to_i32_done");
        let sign_label = self.unique_label("rt_f32_to_i32_sign");
        let sign_done_label = self.unique_label("rt_f32_to_i32_sign_done");
        let exp_low_label = self.unique_label("rt_f32_to_i32_exp_low");
        let exp_done_label = self.unique_label("rt_f32_to_i32_exp_done");
        let left_label = self.unique_label("rt_f32_to_i32_left");
        let right_label = self.unique_label("rt_f32_to_i32_right");
        let shift_done_label = self.unique_label("rt_f32_to_i32_shift_done");
        let negate_label = self.unique_label("rt_f32_to_i32_neg");
        let finish_label = self.unique_label("rt_f32_to_i32_finish");

        self.clear_current_frame_slot(int_offset, i32_ty);
        self.clear_current_frame_slot(flag_offset, u8_ty);
        self.emit_current_frame_nonzero_branch(
            raw_offset,
            Type::new(ScalarType::F32),
            &nonzero_label,
            &done_label,
        );
        self.program.push(AsmLine::Label(nonzero_label));

        if signed {
            self.branch_on_current_frame_bit(raw_offset + 3, 7, &sign_label, &sign_done_label);
            self.program.push(AsmLine::Label(sign_label));
            self.set_current_frame_bit(flag_offset, 0);
            self.program.push(AsmLine::Label(sign_done_label));
        } else {
            self.branch_on_current_frame_bit(raw_offset + 3, 7, &done_label, &sign_done_label);
            self.program.push(AsmLine::Label(sign_done_label));
        }

        self.load_current_frame_byte_to_w(raw_offset + 3);
        self.program.push(AsmLine::Instr(AsmInstr::Andlw(0x7F)));
        self.store_w_to_current_frame_byte(exp_offset);
        self.shift_current_frame_value_left(exp_offset, u8_ty);
        self.branch_on_current_frame_bit(raw_offset + 2, 7, &exp_low_label, &exp_done_label);
        self.program.push(AsmLine::Label(exp_low_label));
        self.set_current_frame_bit(exp_offset, 0);
        self.program.push(AsmLine::Label(exp_done_label));

        self.copy_current_frame_bytes(raw_offset, int_offset, 2);
        self.load_current_frame_byte_to_w(raw_offset + 2);
        self.program.push(AsmLine::Instr(AsmInstr::Andlw(0x7F)));
        self.program.push(AsmLine::Instr(AsmInstr::Iorlw(0x80)));
        self.store_w_to_current_frame_byte(int_offset + 2);
        self.emit_const_to_w(0);
        self.store_w_to_current_frame_byte(int_offset + 3);

        self.emit_const_to_w(base_exp);
        self.store_w_to_current_frame_byte(const_offset);
        self.emit_current_frame_unsigned_ge_branch(
            exp_offset,
            const_offset,
            u8_ty,
            &left_label,
            &right_label,
        );

        self.program.push(AsmLine::Label(left_label));
        self.copy_current_frame_bytes(exp_offset, count_offset, 1);
        self.sub_current_frame_value_from_slot(const_offset, count_offset, u8_ty);
        self.emit_shift_current_frame_by_count(
            int_offset,
            i32_ty,
            count_offset,
            false,
            &shift_done_label,
        );

        self.program.push(AsmLine::Label(right_label));
        self.copy_current_frame_bytes(const_offset, count_offset, 1);
        self.sub_current_frame_value_from_slot(exp_offset, count_offset, u8_ty);
        self.emit_shift_current_frame_by_count(
            int_offset,
            i32_ty,
            count_offset,
            true,
            &shift_done_label,
        );

        self.program.push(AsmLine::Label(shift_done_label));
        if signed {
            self.branch_on_current_frame_bit(flag_offset, 0, &negate_label, &finish_label);
            self.program.push(AsmLine::Label(negate_label));
            self.negate_current_frame_value(int_offset, i32_ty);
        }
        self.program.push(AsmLine::Label(finish_label));
        self.program.push(AsmLine::Label(done_label));
    }

    /// Converts raw little-endian IEEE f32 at `raw_offset` to signed Q16.16 at `q_offset`.
    #[allow(clippy::too_many_arguments)]
    fn emit_float_to_q16_frame(
        &mut self,
        raw_offset: u16,
        q_offset: u16,
        _mant_offset: u16,
        exp_offset: u16,
        count_offset: u16,
        const_offset: u16,
        flag_offset: u16,
    ) {
        let q_ty = Type::new(ScalarType::I32);
        let u8_ty = Type::new(ScalarType::U8);
        let nonzero_label = self.unique_label("rt_f32_to_q16_nonzero");
        let done_label = self.unique_label("rt_f32_to_q16_done");
        let sign_label = self.unique_label("rt_f32_to_q16_sign");
        let sign_done_label = self.unique_label("rt_f32_to_q16_sign_done");
        let exp_low_label = self.unique_label("rt_f32_to_q16_exp_low");
        let exp_done_label = self.unique_label("rt_f32_to_q16_exp_done");
        let left_label = self.unique_label("rt_f32_to_q16_left");
        let right_label = self.unique_label("rt_f32_to_q16_right");
        let shift_done_label = self.unique_label("rt_f32_to_q16_shift_done");
        let negate_label = self.unique_label("rt_f32_to_q16_neg");
        let finish_label = self.unique_label("rt_f32_to_q16_finish");

        self.clear_current_frame_slot(q_offset, q_ty);
        self.clear_current_frame_slot(flag_offset, u8_ty);
        self.emit_current_frame_nonzero_branch(
            raw_offset,
            Type::new(ScalarType::F32),
            &nonzero_label,
            &done_label,
        );
        self.program.push(AsmLine::Label(nonzero_label));

        self.branch_on_current_frame_bit(raw_offset + 3, 7, &sign_label, &sign_done_label);
        self.program.push(AsmLine::Label(sign_label));
        self.set_current_frame_bit(flag_offset, 0);
        self.program.push(AsmLine::Label(sign_done_label));

        self.load_current_frame_byte_to_w(raw_offset + 3);
        self.program.push(AsmLine::Instr(AsmInstr::Andlw(0x7F)));
        self.store_w_to_current_frame_byte(exp_offset);
        self.shift_current_frame_value_left(exp_offset, u8_ty);
        self.branch_on_current_frame_bit(raw_offset + 2, 7, &exp_low_label, &exp_done_label);
        self.program.push(AsmLine::Label(exp_low_label));
        self.set_current_frame_bit(exp_offset, 0);
        self.program.push(AsmLine::Label(exp_done_label));

        self.copy_current_frame_bytes(raw_offset, q_offset, 2);
        self.load_current_frame_byte_to_w(raw_offset + 2);
        self.program.push(AsmLine::Instr(AsmInstr::Andlw(0x7F)));
        self.program.push(AsmLine::Instr(AsmInstr::Iorlw(0x80)));
        self.store_w_to_current_frame_byte(q_offset + 2);
        self.emit_const_to_w(0);
        self.store_w_to_current_frame_byte(q_offset + 3);

        self.emit_const_to_w(134);
        self.store_w_to_current_frame_byte(const_offset);
        self.emit_current_frame_unsigned_ge_branch(
            exp_offset,
            const_offset,
            u8_ty,
            &left_label,
            &right_label,
        );

        self.program.push(AsmLine::Label(left_label));
        self.copy_current_frame_bytes(exp_offset, count_offset, 1);
        self.sub_current_frame_value_from_slot(const_offset, count_offset, u8_ty);
        self.emit_shift_current_frame_by_count(
            q_offset,
            q_ty,
            count_offset,
            false,
            &shift_done_label,
        );

        self.program.push(AsmLine::Label(right_label));
        self.copy_current_frame_bytes(const_offset, count_offset, 1);
        self.sub_current_frame_value_from_slot(exp_offset, count_offset, u8_ty);
        self.emit_shift_current_frame_by_count(
            q_offset,
            q_ty,
            count_offset,
            true,
            &shift_done_label,
        );

        self.program.push(AsmLine::Label(shift_done_label));
        self.branch_on_current_frame_bit(flag_offset, 0, &negate_label, &finish_label);
        self.program.push(AsmLine::Label(negate_label));
        self.negate_current_frame_value(q_offset, q_ty);
        self.program.push(AsmLine::Label(finish_label));
        self.program.push(AsmLine::Label(done_label));
    }

    /// Converts signed Q16.16 at `q_offset` to raw little-endian IEEE f32 at `raw_offset`.
    fn emit_q16_to_float_frame(
        &mut self,
        q_offset: u16,
        raw_offset: u16,
        mant_offset: u16,
        exp_offset: u16,
        count_offset: u16,
        flag_offset: u16,
    ) {
        let q_ty = Type::new(ScalarType::I32);
        let u8_ty = Type::new(ScalarType::U8);
        let nonzero_label = self.unique_label("rt_q16_to_f32_nonzero");
        let done_label = self.unique_label("rt_q16_to_f32_done");
        let sign_label = self.unique_label("rt_q16_to_f32_sign");
        let sign_done_label = self.unique_label("rt_q16_to_f32_sign_done");
        let pack_label = self.unique_label("rt_q16_to_f32_pack");
        let high_loop_label = self.unique_label("rt_q16_to_f32_high_loop");
        let high_body_label = self.unique_label("rt_q16_to_f32_high_body");
        let low_loop_label = self.unique_label("rt_q16_to_f32_low_loop");
        let low_body_label = self.unique_label("rt_q16_to_f32_low_body");

        self.clear_current_frame_slot(raw_offset, Type::new(ScalarType::F32));
        self.clear_current_frame_slot(flag_offset, u8_ty);
        self.emit_current_frame_nonzero_branch(q_offset, q_ty, &nonzero_label, &done_label);
        self.program.push(AsmLine::Label(nonzero_label));
        self.branch_on_current_frame_bit(q_offset + 3, 7, &sign_label, &sign_done_label);
        self.program.push(AsmLine::Label(sign_label));
        self.set_current_frame_bit(flag_offset, 0);
        self.negate_current_frame_value(q_offset, q_ty);
        self.program.push(AsmLine::Label(sign_done_label));

        self.copy_current_frame_bytes(q_offset, mant_offset, 4);
        self.emit_const_to_w(134);
        self.store_w_to_current_frame_byte(exp_offset);

        self.program.push(AsmLine::Label(high_loop_label.clone()));
        self.emit_current_frame_nonzero_branch(
            mant_offset + 3,
            u8_ty,
            &high_body_label,
            &low_loop_label,
        );
        self.program.push(AsmLine::Label(high_body_label));
        self.shift_current_frame_value_right(mant_offset, q_ty, false);
        self.emit_const_to_w(1);
        self.store_w_to_current_frame_byte(count_offset);
        self.add_current_frame_value_into_slot(count_offset, exp_offset, u8_ty);
        self.jump_to_label(&high_loop_label);

        self.program.push(AsmLine::Label(low_loop_label.clone()));
        self.branch_on_current_frame_bit(mant_offset + 2, 7, &pack_label, &low_body_label);
        self.program.push(AsmLine::Label(low_body_label));
        self.shift_current_frame_value_left(mant_offset, q_ty);
        self.decrement_current_frame_value(exp_offset, u8_ty);
        self.jump_to_label(&low_loop_label);

        self.program.push(AsmLine::Label(pack_label));
        self.copy_current_frame_bytes(mant_offset, raw_offset, 2);
        self.load_current_frame_byte_to_w(mant_offset + 2);
        self.program.push(AsmLine::Instr(AsmInstr::Andlw(0x7F)));
        self.store_w_to_current_frame_byte(raw_offset + 2);
        let exp_low_label = self.unique_label("rt_q16_to_f32_exp_low");
        let exp_done_label = self.unique_label("rt_q16_to_f32_exp_done");
        self.branch_on_current_frame_bit(exp_offset, 0, &exp_low_label, &exp_done_label);
        self.program.push(AsmLine::Label(exp_low_label));
        self.set_current_frame_bit(raw_offset + 2, 7);
        self.program.push(AsmLine::Label(exp_done_label));
        self.copy_current_frame_bytes(exp_offset, count_offset, 1);
        self.shift_current_frame_value_right(count_offset, u8_ty, false);
        self.load_current_frame_byte_to_w(count_offset);
        self.store_w_to_current_frame_byte(raw_offset + 3);
        let sign_set_label = self.unique_label("rt_q16_to_f32_sign_set");
        let sign_finish_label = self.unique_label("rt_q16_to_f32_sign_finish");
        self.branch_on_current_frame_bit(flag_offset, 0, &sign_set_label, &sign_finish_label);
        self.program.push(AsmLine::Label(sign_set_label));
        self.set_current_frame_bit(raw_offset + 3, 7);
        self.program.push(AsmLine::Label(sign_finish_label));
        self.program.push(AsmLine::Label(done_label));
    }

    /// Shifts one frame value until an 8-bit count reaches zero.
    fn emit_shift_current_frame_by_count(
        &mut self,
        offset: u16,
        ty: Type,
        count_offset: u16,
        right: bool,
        done_label: &str,
    ) {
        let loop_label = self.unique_label("rt_shift_count_loop");
        let body_label = self.unique_label("rt_shift_count_body");
        self.program.push(AsmLine::Label(loop_label.clone()));
        self.emit_current_frame_nonzero_branch(
            count_offset,
            Type::new(ScalarType::U8),
            &body_label,
            done_label,
        );
        self.program.push(AsmLine::Label(body_label));
        if right {
            self.shift_current_frame_value_right(offset, ty, false);
        } else {
            self.shift_current_frame_value_left(offset, ty);
        }
        self.decrement_current_frame_value(count_offset, Type::new(ScalarType::U8));
        self.jump_to_label(&loop_label);
    }

    /// Emits Q8.8 fixed multiply: `(raw_a * raw_b) >> 8`, returning the low 16 result bytes.
    fn emit_fixed_q8_8_mul_helper(&mut self, ty: Type, local_base: u16, signed: bool) {
        let wide_ty = Type::new(ScalarType::U32);
        let arg0_offset = 0u16;
        let arg1_offset = ty.byte_width() as u16;
        let lhs32_offset = local_base;
        let rhs32_offset = lhs32_offset + 4;
        let work32_offset = rhs32_offset + 4;
        let count_offset = work32_offset + 4;
        let flag_offset = count_offset + 1;

        if signed {
            self.clear_current_frame_slot(flag_offset, Type::new(ScalarType::U8));
            self.emit_runtime_negate_if_signed(arg0_offset, ty, flag_offset, 0x01);
            self.emit_runtime_negate_if_signed(arg1_offset, ty, flag_offset, 0x01);
        }
        self.clear_current_frame_slot(lhs32_offset, wide_ty);
        self.clear_current_frame_slot(rhs32_offset, wide_ty);
        self.clear_current_frame_slot(work32_offset, wide_ty);
        self.copy_current_frame_bytes(arg0_offset, lhs32_offset, ty.byte_width());
        self.copy_current_frame_bytes(arg1_offset, rhs32_offset, ty.byte_width());
        self.emit_const_to_w(wide_ty.bit_width() as u8);
        self.store_w_to_current_frame_byte(count_offset);
        self.emit_unsigned_mul_core(
            lhs32_offset,
            rhs32_offset,
            work32_offset,
            count_offset,
            wide_ty,
        );
        for _ in 0..8 {
            self.shift_current_frame_value_right(work32_offset, wide_ty, false);
        }
        if signed {
            let negate_label = self.unique_label("rt_qmul_neg");
            let done_label = self.unique_label("rt_qmul_done");
            self.branch_on_current_frame_bit(flag_offset, 0, &negate_label, &done_label);
            self.program.push(AsmLine::Label(negate_label));
            self.negate_current_frame_value(work32_offset, ty);
            self.program.push(AsmLine::Label(done_label));
        }
        self.emit_return_current_frame_value(work32_offset, ty);
    }

    /// Emits Q8.8 fixed divide: `(raw_a << 8) / raw_b`, returning the low 16 quotient bytes.
    fn emit_fixed_q8_8_div_helper(&mut self, ty: Type, local_base: u16, signed: bool) {
        let wide_ty = Type::new(ScalarType::U32);
        let arg0_offset = 0u16;
        let arg1_offset = ty.byte_width() as u16;
        let dividend32_offset = local_base;
        let divisor32_offset = dividend32_offset + 4;
        let remainder32_offset = divisor32_offset + 4;
        let count_offset = remainder32_offset + 4;
        let flag_offset = count_offset + 1;
        let core_label = self.unique_label("rt_qdiv_core");
        let zero_label = self.unique_label("rt_qdiv_zero");
        let finish_label = self.unique_label("rt_qdiv_finish");

        if signed {
            self.clear_current_frame_slot(flag_offset, Type::new(ScalarType::U8));
            self.emit_runtime_negate_if_signed(arg0_offset, ty, flag_offset, 0x01);
            self.emit_runtime_negate_if_signed(arg1_offset, ty, flag_offset, 0x01);
        }
        self.clear_current_frame_slot(dividend32_offset, wide_ty);
        self.clear_current_frame_slot(divisor32_offset, wide_ty);
        self.clear_current_frame_slot(remainder32_offset, wide_ty);
        self.copy_current_frame_bytes(arg0_offset, dividend32_offset, ty.byte_width());
        self.copy_current_frame_bytes(arg1_offset, divisor32_offset, ty.byte_width());
        for _ in 0..8 {
            self.shift_current_frame_value_left(dividend32_offset, wide_ty);
        }
        self.emit_const_to_w(wide_ty.bit_width() as u8);
        self.store_w_to_current_frame_byte(count_offset);
        self.emit_current_frame_nonzero_branch(divisor32_offset, wide_ty, &core_label, &zero_label);
        self.program.push(AsmLine::Label(core_label));
        self.emit_unsigned_divmod_core(
            dividend32_offset,
            divisor32_offset,
            remainder32_offset,
            count_offset,
            wide_ty,
            &finish_label,
        );
        self.program.push(AsmLine::Label(zero_label));
        self.clear_current_frame_slot(dividend32_offset, wide_ty);
        self.clear_current_frame_slot(remainder32_offset, wide_ty);
        self.jump_to_label(&finish_label);
        self.program.push(AsmLine::Label(finish_label));
        if signed {
            let negate_label = self.unique_label("rt_qdiv_neg");
            let done_label = self.unique_label("rt_qdiv_done");
            self.branch_on_current_frame_bit(flag_offset, 0, &negate_label, &done_label);
            self.program.push(AsmLine::Label(negate_label));
            self.negate_current_frame_value(dividend32_offset, ty);
            self.program.push(AsmLine::Label(done_label));
        }
        self.emit_return_current_frame_value(dividend32_offset, ty);
    }

    /// Emits Q16.16 fixed multiply through 16x16 partial products without a public 64-bit type.
    fn emit_fixed_q16_16_mul_helper(&mut self, ty: Type, local_base: u16, signed: bool) {
        let work_ty = Type::new(ScalarType::U32);
        let arg0_offset = 0u16;
        let arg1_offset = ty.byte_width() as u16;
        let result32_offset = local_base;
        let multiplicand32_offset = result32_offset + 4;
        let multiplier16_offset = multiplicand32_offset + 4;
        let work32_offset = multiplier16_offset + 2;
        let count_offset = work32_offset + 4;
        let flag_offset = count_offset + 1;
        let core_label = self.unique_label("rt_q16mul16_core");
        let done_label = self.unique_label("rt_q16mul_done");

        if signed {
            self.clear_current_frame_slot(flag_offset, Type::new(ScalarType::U8));
            self.emit_runtime_negate_if_signed(arg0_offset, ty, flag_offset, 0x01);
            self.emit_runtime_negate_if_signed(arg1_offset, ty, flag_offset, 0x01);
        }
        self.clear_current_frame_slot(result32_offset, work_ty);
        self.emit_fixed_q16_16_mul_partial(
            arg0_offset,
            arg1_offset,
            -16,
            multiplicand32_offset,
            multiplier16_offset,
            work32_offset,
            &core_label,
            result32_offset,
        );
        self.emit_fixed_q16_16_mul_partial(
            arg0_offset + 2,
            arg1_offset,
            0,
            multiplicand32_offset,
            multiplier16_offset,
            work32_offset,
            &core_label,
            result32_offset,
        );
        self.emit_fixed_q16_16_mul_partial(
            arg0_offset,
            arg1_offset + 2,
            0,
            multiplicand32_offset,
            multiplier16_offset,
            work32_offset,
            &core_label,
            result32_offset,
        );
        self.emit_fixed_q16_16_mul_partial(
            arg0_offset + 2,
            arg1_offset + 2,
            16,
            multiplicand32_offset,
            multiplier16_offset,
            work32_offset,
            &core_label,
            result32_offset,
        );
        if signed {
            let negate_label = self.unique_label("rt_q16mul_neg");
            let sign_done_label = self.unique_label("rt_q16mul_sign_done");
            self.restore_code_page_after_call();
            self.branch_on_current_frame_bit(flag_offset, 0, &negate_label, &sign_done_label);
            self.program.push(AsmLine::Label(negate_label));
            self.negate_current_frame_value(result32_offset, ty);
            self.program.push(AsmLine::Label(sign_done_label));
        }
        self.jump_to_label(&done_label);
        self.program.push(AsmLine::Label(core_label));
        self.emit_fixed_q16_16_mul16_core(
            multiplicand32_offset,
            multiplier16_offset,
            work32_offset,
            count_offset,
        );
        self.program.push(AsmLine::Label(done_label));
        self.emit_return_current_frame_value(result32_offset, ty);
    }

    /// Emits one Q16.16 multiply partial term into the accumulated raw 32-bit result.
    #[allow(clippy::too_many_arguments)]
    fn emit_fixed_q16_16_mul_partial(
        &mut self,
        lhs_src_offset: u16,
        rhs_src_offset: u16,
        product_shift: i8,
        multiplicand32_offset: u16,
        multiplier16_offset: u16,
        work32_offset: u16,
        core_label: &str,
        result32_offset: u16,
    ) {
        let work_ty = Type::new(ScalarType::U32);
        self.clear_current_frame_slot(multiplicand32_offset, work_ty);
        self.clear_current_frame_slot(multiplier16_offset, Type::new(ScalarType::U16));
        self.copy_current_frame_bytes(lhs_src_offset, multiplicand32_offset, 2);
        self.copy_current_frame_bytes(rhs_src_offset, multiplier16_offset, 2);
        self.program
            .push(AsmLine::Instr(AsmInstr::SetPage(core_label.to_string())));
        self.program
            .push(AsmLine::Instr(AsmInstr::Call(core_label.to_string())));
        self.restore_code_page_after_call();
        self.current_bank = UNKNOWN_BANK;
        if product_shift < 0 {
            self.shift_current_frame_value_right_16(work32_offset);
        } else {
            for _ in 0..product_shift / 16 {
                self.shift_current_frame_value_left_16(work32_offset);
            }
        }
        self.add_current_frame_value_into_slot(work32_offset, result32_offset, work_ty);
    }

    /// Local helper subroutine: 16x16 unsigned multiply into one 32-bit work slot.
    fn emit_fixed_q16_16_mul16_core(
        &mut self,
        multiplicand32_offset: u16,
        multiplier16_offset: u16,
        work32_offset: u16,
        count_offset: u16,
    ) {
        let work_ty = Type::new(ScalarType::U32);
        let multiplier_ty = Type::new(ScalarType::U16);
        let loop_label = self.unique_label("rt_q16mul16_loop");
        let body_label = self.unique_label("rt_q16mul16_body");
        let add_label = self.unique_label("rt_q16mul16_add");
        let next_label = self.unique_label("rt_q16mul16_next");
        let done_label = self.unique_label("rt_q16mul16_done");

        self.clear_current_frame_slot(work32_offset, work_ty);
        self.emit_const_to_w(16);
        self.store_w_to_current_frame_byte(count_offset);
        self.program.push(AsmLine::Label(loop_label.clone()));
        self.emit_current_frame_nonzero_branch(
            count_offset,
            Type::new(ScalarType::U8),
            &body_label,
            &done_label,
        );
        self.program.push(AsmLine::Label(body_label));
        self.branch_on_current_frame_bit(multiplier16_offset, 0, &add_label, &next_label);
        self.program.push(AsmLine::Label(add_label));
        self.add_current_frame_value_into_slot(multiplicand32_offset, work32_offset, work_ty);
        self.program.push(AsmLine::Label(next_label));
        self.shift_current_frame_value_left(multiplicand32_offset, work_ty);
        self.shift_current_frame_value_right(multiplier16_offset, multiplier_ty, false);
        self.decrement_current_frame_value(count_offset, Type::new(ScalarType::U8));
        self.restore_code_page_after_call();
        self.branch_to_label(&loop_label);
        self.program.push(AsmLine::Label(done_label));
        self.program.push(AsmLine::Instr(AsmInstr::Return));
    }

    /// Emits Q16.16 fixed divide using a 32-bit divide plus 16 fractional restoring steps.
    fn emit_fixed_q16_16_div_helper(&mut self, ty: Type, local_base: u16, signed: bool) {
        let work_ty = Type::new(ScalarType::U32);
        let arg0_offset = 0u16;
        let arg1_offset = ty.byte_width() as u16;
        let quotient32_offset = local_base;
        let divisor32_offset = quotient32_offset + 4;
        let remainder32_offset = divisor32_offset + 4;
        let count_offset = remainder32_offset + 4;
        let flag_offset = count_offset + 1;
        let core_label = self.unique_label("rt_q16div_core");
        let zero_label = self.unique_label("rt_q16div_zero");
        let fraction_label = self.unique_label("rt_q16div_fraction");
        let finish_label = self.unique_label("rt_q16div_finish");

        if signed {
            self.clear_current_frame_slot(flag_offset, Type::new(ScalarType::U8));
            self.emit_runtime_negate_if_signed(arg0_offset, ty, flag_offset, 0x01);
            self.emit_runtime_negate_if_signed(arg1_offset, ty, flag_offset, 0x01);
        }
        self.copy_current_frame_bytes(arg0_offset, quotient32_offset, ty.byte_width());
        self.copy_current_frame_bytes(arg1_offset, divisor32_offset, ty.byte_width());
        self.clear_current_frame_slot(remainder32_offset, work_ty);
        self.emit_const_to_w(work_ty.bit_width() as u8);
        self.store_w_to_current_frame_byte(count_offset);
        self.emit_current_frame_nonzero_branch(divisor32_offset, work_ty, &core_label, &zero_label);
        self.program.push(AsmLine::Label(core_label));
        self.emit_unsigned_divmod_core(
            quotient32_offset,
            divisor32_offset,
            remainder32_offset,
            count_offset,
            work_ty,
            &fraction_label,
        );
        self.program.push(AsmLine::Label(zero_label));
        self.clear_current_frame_slot(quotient32_offset, work_ty);
        self.clear_current_frame_slot(remainder32_offset, work_ty);
        self.jump_to_label(&finish_label);
        self.program.push(AsmLine::Label(fraction_label));
        self.emit_const_to_w(16);
        self.store_w_to_current_frame_byte(count_offset);
        let loop_label = self.unique_label("rt_q16div_frac_loop");
        let body_label = self.unique_label("rt_q16div_frac_body");
        self.program.push(AsmLine::Label(loop_label.clone()));
        self.emit_current_frame_nonzero_branch(
            count_offset,
            Type::new(ScalarType::U8),
            &body_label,
            &finish_label,
        );
        self.program.push(AsmLine::Label(body_label));
        {
            self.shift_current_frame_value_left(quotient32_offset, work_ty);
            self.shift_current_frame_value_left(remainder32_offset, work_ty);
            let subtract_label = self.unique_label("rt_q16div_sub");
            let next_label = self.unique_label("rt_q16div_next");
            self.branch_if_bit_set(low7(STATUS_ADDR), STATUS_C_BIT, &subtract_label);
            self.emit_current_frame_unsigned_ge_branch(
                remainder32_offset,
                divisor32_offset,
                work_ty,
                &subtract_label,
                &next_label,
            );
            self.program.push(AsmLine::Label(subtract_label));
            self.sub_current_frame_value_from_slot(divisor32_offset, remainder32_offset, work_ty);
            self.set_current_frame_bit(quotient32_offset, 0);
            self.program.push(AsmLine::Label(next_label));
        }
        self.decrement_current_frame_value(count_offset, Type::new(ScalarType::U8));
        self.restore_code_page_after_call();
        self.branch_to_label(&loop_label);
        self.program.push(AsmLine::Label(finish_label));
        if signed {
            let negate_label = self.unique_label("rt_q16div_neg");
            let done_label = self.unique_label("rt_q16div_done");
            self.branch_on_current_frame_bit(flag_offset, 0, &negate_label, &done_label);
            self.program.push(AsmLine::Label(negate_label));
            self.negate_current_frame_value(quotient32_offset, ty);
            self.program.push(AsmLine::Label(done_label));
        }
        self.emit_return_current_frame_value(quotient32_offset, ty);
    }

    /// Applies a 16-bit logical left shift to one 32-bit frame slot using byte moves.
    fn shift_current_frame_value_left_16(&mut self, offset: u16) {
        self.load_current_frame_byte_to_w(offset + 1);
        self.store_w_to_current_frame_byte(offset + 3);
        self.load_current_frame_byte_to_w(offset);
        self.store_w_to_current_frame_byte(offset + 2);
        self.emit_const_to_w(0);
        self.store_w_to_current_frame_byte(offset);
        self.emit_const_to_w(0);
        self.store_w_to_current_frame_byte(offset + 1);
    }

    /// Applies a 16-bit logical right shift to one 32-bit frame slot using byte moves.
    fn shift_current_frame_value_right_16(&mut self, offset: u16) {
        self.load_current_frame_byte_to_w(offset + 2);
        self.store_w_to_current_frame_byte(offset);
        self.load_current_frame_byte_to_w(offset + 3);
        self.store_w_to_current_frame_byte(offset + 1);
        self.emit_const_to_w(0);
        self.store_w_to_current_frame_byte(offset + 2);
        self.emit_const_to_w(0);
        self.store_w_to_current_frame_byte(offset + 3);
    }

    /// Emits compact Phase 35 signed Q16.16 wrappers through unsigned Q16.16 helpers.
    fn emit_q16_16_small_signed_wrapper(
        &mut self,
        unsigned_helper: RuntimeHelper,
        arg0_offset: u16,
        arg1_offset: u16,
        result_offset: u16,
        flag_offset: u16,
        result_ty: Type,
    ) {
        self.clear_current_frame_slot(flag_offset, Type::new(ScalarType::U8));
        self.emit_runtime_negate_if_signed(arg0_offset, result_ty, flag_offset, 0x01);
        self.emit_runtime_negate_if_signed(arg1_offset, result_ty, flag_offset, 0x01);
        self.emit_call_binary_runtime_helper_32(
            unsigned_helper,
            arg0_offset,
            arg1_offset,
            result_offset,
        );

        let negate_label = self.unique_label("rt_q16_small_wrap_neg");
        let done_label = self.unique_label("rt_q16_small_wrap_done");
        self.branch_on_current_frame_bit(flag_offset, 0, &negate_label, &done_label);
        self.program.push(AsmLine::Label(negate_label));
        self.negate_current_frame_value(result_offset, result_ty);
        self.program.push(AsmLine::Label(done_label));
        self.emit_return_current_frame_value(result_offset, result_ty);
    }

    /// Copies contiguous bytes inside the active helper frame.
    fn copy_current_frame_bytes(&mut self, src_offset: u16, dst_offset: u16, width: usize) {
        for byte in 0..width {
            self.load_current_frame_byte_to_w(src_offset + byte as u16);
            self.store_w_to_current_frame_byte(dst_offset + byte as u16);
        }
    }

    /// Emits the common stack-first runtime-helper prologue.
    fn emit_runtime_prologue(&mut self, info: RuntimeHelperInfo) {
        self.emit_stack_growth_check(
            info.frame_bytes,
            &format!("runtime helper frame {}", info.label),
        );
        self.load_addr_to_w(self.layout.helpers.frame_ptr.lo);
        self.store_w_to_addr(self.layout.helpers.scratch0);
        self.load_addr_to_w(self.layout.helpers.frame_ptr.hi);
        self.store_w_to_addr(self.layout.helpers.scratch1);
        self.copy_pair_with_signed_offset(
            self.layout.helpers.stack_ptr,
            self.layout.helpers.frame_ptr,
            negate_u16(info.arg_bytes),
        );
        self.load_addr_to_w(self.layout.helpers.scratch0);
        self.push_w();
        self.load_addr_to_w(self.layout.helpers.scratch1);
        self.push_w();
        if info.local_bytes != 0 {
            self.add_immediate_to_pair(self.layout.helpers.stack_ptr, info.local_bytes);
        }
    }

    /// Emits the common stack-first runtime-helper epilogue.
    fn emit_runtime_epilogue(&mut self, info: RuntimeHelperInfo) {
        self.store_w_to_addr(self.layout.helpers.w_save);
        self.load_current_frame_byte_to_w(info.arg_bytes);
        self.store_w_to_addr(self.layout.helpers.scratch0);
        self.load_current_frame_byte_to_w(info.arg_bytes + 1);
        self.store_w_to_addr(self.layout.helpers.scratch1);
        self.copy_pair_with_signed_offset(
            self.layout.helpers.frame_ptr,
            self.layout.helpers.stack_ptr,
            info.arg_bytes,
        );
        self.load_addr_to_w(self.layout.helpers.scratch0);
        self.store_w_to_addr(self.layout.helpers.frame_ptr.lo);
        self.load_addr_to_w(self.layout.helpers.scratch1);
        self.store_w_to_addr(self.layout.helpers.frame_ptr.hi);
        self.load_addr_to_w(self.layout.helpers.w_save);
    }

    /// Emits the compact Phase 34 32-bit div/mod core used by the `small` profile.
    fn emit_u32_divmod_core_helper(&mut self, local_base: u16) {
        let ty = Type::new(ScalarType::U32);
        let arg0_offset = 0u16;
        let arg1_offset = 4u16;
        let mode_offset = 8u16;
        let remainder_offset = local_base;
        let count_offset = remainder_offset + 4;
        let core_label = self.unique_label("rt_u32_divmod_core");
        let zero_label = self.unique_label("rt_u32_divmod_zero");
        let finish_label = self.unique_label("rt_u32_divmod_finish");
        let modulo_label = self.unique_label("rt_u32_divmod_mod");
        let quotient_label = self.unique_label("rt_u32_divmod_quot");

        self.clear_current_frame_slot(remainder_offset, ty);
        self.emit_const_to_w(32);
        self.store_w_to_current_frame_byte(count_offset);
        self.emit_current_frame_nonzero_branch(arg1_offset, ty, &core_label, &zero_label);
        self.program.push(AsmLine::Label(core_label));
        self.emit_unsigned_divmod_core(
            arg0_offset,
            arg1_offset,
            remainder_offset,
            count_offset,
            ty,
            &finish_label,
        );
        self.program.push(AsmLine::Label(zero_label));
        self.clear_current_frame_slot(arg0_offset, ty);
        self.clear_current_frame_slot(remainder_offset, ty);
        self.jump_to_label(&finish_label);
        self.program.push(AsmLine::Label(finish_label));
        self.emit_current_frame_nonzero_branch(
            mode_offset,
            Type::new(ScalarType::U8),
            &modulo_label,
            &quotient_label,
        );
        self.program.push(AsmLine::Label(quotient_label));
        self.emit_return_current_frame_value(arg0_offset, ty);
        let done_label = self.unique_label("rt_u32_divmod_done");
        self.jump_to_label(&done_label);
        self.program.push(AsmLine::Label(modulo_label));
        self.emit_return_current_frame_value(remainder_offset, ty);
        self.program.push(AsmLine::Label(done_label));
    }

    /// Emits compact Phase 34 wrappers that share `__rt_u32_divmod_core`.
    fn emit_u32_divmod_small_wrapper(
        &mut self,
        helper: RuntimeHelper,
        arg0_offset: u16,
        arg1_offset: u16,
        result_offset: u16,
        flag_offset: u16,
        signed: bool,
    ) {
        let ty = Type::new(ScalarType::U32);
        if signed {
            self.clear_current_frame_slot(flag_offset, Type::new(ScalarType::U8));
            self.emit_runtime_set_flag_if_signed(arg0_offset, ty, flag_offset, 0x02);
            self.emit_runtime_negate_if_signed(arg0_offset, ty, flag_offset, 0x01);
            self.emit_runtime_negate_if_signed(arg1_offset, ty, flag_offset, 0x01);
        }

        let mode = u8::from(matches!(
            helper,
            RuntimeHelper::ModU32 | RuntimeHelper::ModI32
        ));
        self.emit_call_u32_divmod_core(arg0_offset, arg1_offset, mode, result_offset);

        if signed {
            let negate_label = self.unique_label("rt_u32_divmod_wrap_neg");
            let done_label = self.unique_label("rt_u32_divmod_wrap_done");
            let flag_bit = if matches!(helper, RuntimeHelper::DivI32) {
                0
            } else {
                1
            };
            self.branch_on_current_frame_bit(flag_offset, flag_bit, &negate_label, &done_label);
            self.program.push(AsmLine::Label(negate_label));
            self.negate_current_frame_value(result_offset, ty);
            self.program.push(AsmLine::Label(done_label));
        }

        self.emit_return_current_frame_value(result_offset, ty);
    }

    /// Calls the compact unsigned 32-bit div/mod primitive from another helper.
    fn emit_call_u32_divmod_core(
        &mut self,
        arg0_offset: u16,
        arg1_offset: u16,
        mode: u8,
        result_offset: u16,
    ) {
        let core = RuntimeHelper::U32DivModCore;
        let info = core.info();
        self.emit_stack_growth_check(info.arg_bytes, "runtime helper __rt_u32_divmod_core args");
        for byte in 0..4u16 {
            self.load_current_frame_byte_to_w(arg0_offset + byte);
            self.push_w();
        }
        for byte in 0..4u16 {
            self.load_current_frame_byte_to_w(arg1_offset + byte);
            self.push_w();
        }
        self.emit_const_to_w(mode);
        self.push_w();
        self.program
            .push(AsmLine::Instr(AsmInstr::SetPage(info.label.to_string())));
        self.program
            .push(AsmLine::Instr(AsmInstr::Call(info.label.to_string())));
        self.restore_code_page_after_call();
        self.store_w_to_addr(self.layout.helpers.w_save);
        self.add_immediate_to_pair(self.layout.helpers.stack_ptr, negate_u16(info.arg_bytes));
        self.load_addr_to_w(self.layout.helpers.w_save);
        self.store_w_to_current_frame_byte(result_offset);
        for byte in 1..4usize {
            self.load_return_byte_to_w(byte);
            self.store_w_to_current_frame_byte(result_offset + byte as u16);
        }
    }

    /// Calls one 32-bit binary runtime helper from another runtime helper.
    fn emit_call_binary_runtime_helper_32(
        &mut self,
        helper: RuntimeHelper,
        arg0_offset: u16,
        arg1_offset: u16,
        result_offset: u16,
    ) {
        let info = helper.info();
        debug_assert_eq!(info.arg_bytes, 8);
        self.emit_stack_growth_check(
            info.arg_bytes,
            &format!("runtime helper {} args", info.label),
        );
        for byte in 0..4u16 {
            self.load_current_frame_byte_to_w(arg0_offset + byte);
            self.push_w();
        }
        for byte in 0..4u16 {
            self.load_current_frame_byte_to_w(arg1_offset + byte);
            self.push_w();
        }
        self.program
            .push(AsmLine::Instr(AsmInstr::SetPage(info.label.to_string())));
        self.program
            .push(AsmLine::Instr(AsmInstr::Call(info.label.to_string())));
        self.restore_code_page_after_call();
        self.store_w_to_addr(self.layout.helpers.w_save);
        self.add_immediate_to_pair(self.layout.helpers.stack_ptr, negate_u16(info.arg_bytes));
        self.load_addr_to_w(self.layout.helpers.w_save);
        self.store_w_to_current_frame_byte(result_offset);
        for byte in 1..4usize {
            self.load_return_byte_to_w(byte);
            self.store_w_to_current_frame_byte(result_offset + byte as u16);
        }
    }

    /// Calls one 32-bit unary runtime helper from another runtime helper.
    fn emit_call_unary_runtime_helper_32(
        &mut self,
        helper: RuntimeHelper,
        arg_offset: u16,
        result_offset: u16,
    ) {
        let info = helper.info();
        debug_assert_eq!(info.arg_bytes, 4);
        self.emit_stack_growth_check(
            info.arg_bytes,
            &format!("runtime helper {} args", info.label),
        );
        for byte in 0..4u16 {
            self.load_current_frame_byte_to_w(arg_offset + byte);
            self.push_w();
        }
        self.program
            .push(AsmLine::Instr(AsmInstr::SetPage(info.label.to_string())));
        self.program
            .push(AsmLine::Instr(AsmInstr::Call(info.label.to_string())));
        self.restore_code_page_after_call();
        self.store_w_to_addr(self.layout.helpers.w_save);
        self.add_immediate_to_pair(self.layout.helpers.stack_ptr, negate_u16(info.arg_bytes));
        self.load_addr_to_w(self.layout.helpers.w_save);
        self.store_w_to_current_frame_byte(result_offset);
        for byte in 1..4usize {
            self.load_return_byte_to_w(byte);
            self.store_w_to_current_frame_byte(result_offset + byte as u16);
        }
    }

    /// Emits unsigned shift-and-add multiplication into a local result slot.
    fn emit_unsigned_mul_core(
        &mut self,
        multiplicand_offset: u16,
        multiplier_offset: u16,
        result_offset: u16,
        count_offset: u16,
        ty: Type,
    ) {
        let loop_label = self.unique_label("rt_mul_loop");
        let body_label = self.unique_label("rt_mul_body");
        let add_label = self.unique_label("rt_mul_add");
        let next_label = self.unique_label("rt_mul_next");
        let done_label = self.unique_label("rt_mul_done");
        self.program.push(AsmLine::Label(loop_label.clone()));
        self.emit_current_frame_nonzero_branch(
            count_offset,
            Type::new(ScalarType::U8),
            &body_label,
            &done_label,
        );
        self.program.push(AsmLine::Label(body_label));
        self.branch_on_current_frame_bit(multiplier_offset, 0, &add_label, &next_label);
        self.program.push(AsmLine::Label(add_label));
        self.add_current_frame_value_into_slot(multiplicand_offset, result_offset, ty);
        self.program.push(AsmLine::Label(next_label));
        self.shift_current_frame_value_left(multiplicand_offset, ty);
        self.shift_current_frame_value_right(multiplier_offset, ty, false);
        self.decrement_current_frame_value(count_offset, Type::new(ScalarType::U8));
        self.jump_to_label(&loop_label);
        self.program.push(AsmLine::Label(done_label));
    }

    /// Emits one loop-based restoring-division core that materializes quotient in `arg0`.
    fn emit_unsigned_divmod_core(
        &mut self,
        dividend_offset: u16,
        divisor_offset: u16,
        remainder_offset: u16,
        count_offset: u16,
        ty: Type,
        finish_label: &str,
    ) {
        let loop_label = self.unique_label("rt_div_loop");
        let body_label = self.unique_label("rt_div_body");
        self.program.push(AsmLine::Label(loop_label.clone()));
        self.emit_current_frame_nonzero_branch(
            count_offset,
            Type::new(ScalarType::U8),
            &body_label,
            finish_label,
        );
        self.program.push(AsmLine::Label(body_label));
        self.program.push(AsmLine::Instr(AsmInstr::Bcf {
            f: low7(STATUS_ADDR),
            b: STATUS_C_BIT,
        }));
        self.rotate_current_frame_value_left(dividend_offset, ty);
        self.rotate_current_frame_value_left(remainder_offset, ty);
        let ge_label = self.unique_label("rt_div_ge");
        let next_label = self.unique_label("rt_div_next");
        self.emit_current_frame_unsigned_ge_branch(
            remainder_offset,
            divisor_offset,
            ty,
            &ge_label,
            &next_label,
        );
        self.program.push(AsmLine::Label(ge_label));
        self.sub_current_frame_value_from_slot(divisor_offset, remainder_offset, ty);
        self.set_current_frame_bit(dividend_offset, 0);
        self.program.push(AsmLine::Label(next_label));
        self.decrement_current_frame_value(count_offset, Type::new(ScalarType::U8));
        self.restore_code_page_after_call();
        self.branch_to_label(&loop_label);
    }

    /// Negates a signed arg in place and toggles one helper flag when the sign bit was set.
    fn emit_runtime_negate_if_signed(
        &mut self,
        offset: u16,
        ty: Type,
        flag_offset: u16,
        flag_mask: u8,
    ) {
        let negate_label = self.unique_label("rt_negate");
        let done_label = self.unique_label("rt_negate_done");
        self.branch_on_current_frame_bit(
            offset + (ty.byte_width() - 1) as u16,
            7,
            &negate_label,
            &done_label,
        );
        self.program.push(AsmLine::Label(negate_label));
        self.load_current_frame_byte_to_w(flag_offset);
        self.program
            .push(AsmLine::Instr(AsmInstr::Xorlw(flag_mask)));
        self.store_w_to_current_frame_byte(flag_offset);
        self.negate_current_frame_value(offset, ty);
        self.program.push(AsmLine::Label(done_label));
    }

    /// Sets one helper flag when a signed arg carries a negative sign bit.
    fn emit_runtime_set_flag_if_signed(
        &mut self,
        offset: u16,
        ty: Type,
        flag_offset: u16,
        flag_mask: u8,
    ) {
        let set_label = self.unique_label("rt_setflag");
        let done_label = self.unique_label("rt_setflag_done");
        self.branch_on_current_frame_bit(
            offset + (ty.byte_width() - 1) as u16,
            7,
            &set_label,
            &done_label,
        );
        self.program.push(AsmLine::Label(set_label));
        self.load_current_frame_byte_to_w(flag_offset);
        self.program
            .push(AsmLine::Instr(AsmInstr::Iorlw(flag_mask)));
        self.store_w_to_current_frame_byte(flag_offset);
        self.program.push(AsmLine::Label(done_label));
    }

    /// Clamps a dynamic shift count to the operand bit width to avoid unbounded helper loops.
    fn clamp_runtime_shift_count(&mut self, offset: u16, ty: Type) {
        let clamp_label = self.unique_label("rt_shift_clamp");
        let done_label = self.unique_label("rt_shift_clamp_done");
        let width = ty.bit_width() as u8;

        for byte in 1..ty.byte_width() {
            let next_label = self.unique_label("rt_shift_high_zero");
            self.emit_current_frame_nonzero_branch(
                offset + byte as u16,
                Type::new(ScalarType::U8),
                &clamp_label,
                &next_label,
            );
            self.program.push(AsmLine::Label(next_label));
        }

        self.load_current_frame_byte_to_w(offset);
        self.store_w_to_addr(self.layout.helpers.scratch0);
        self.emit_const_to_w(width);
        self.select_bank(self.layout.helpers.scratch0);
        self.program.push(AsmLine::Instr(AsmInstr::Subwf {
            f: low7(self.layout.helpers.scratch0),
            d: Dest::W,
        }));
        self.branch_if_bit_set(low7(STATUS_ADDR), STATUS_C_BIT, &clamp_label);
        self.jump_to_label(&done_label);
        self.program.push(AsmLine::Label(clamp_label));
        self.emit_const_to_w(width);
        self.store_w_to_current_frame_byte(offset);
        for byte in 1..ty.byte_width() {
            self.emit_const_to_w(0);
            self.store_w_to_current_frame_byte(offset + byte as u16);
        }
        self.program.push(AsmLine::Label(done_label));
    }

    /// Returns the storage classification assigned to one source-level symbol.
    fn symbol_storage(&self, symbol: SymbolId) -> SymbolStorage {
        self.layout.symbol_storage[&symbol]
    }

    /// Returns the lowered type carried by one operand in the current function.
    fn operand_type(&self, function: &IrFunction, operand: Operand) -> Type {
        match operand {
            Operand::Constant(_) => Type::new(ScalarType::U16),
            Operand::Symbol(symbol) => self.symbol_type(symbol),
            Operand::Temp(temp) => function.temp_types[temp],
        }
    }

    /// Returns the frame layout metadata associated with one function symbol.
    fn frame_layout(&self, function_symbol: SymbolId) -> &FrameLayout {
        &self.layout.frames[&function_symbol]
    }

    /// Returns the total argument byte count for one callee signature.
    fn function_arg_bytes(&self, function_symbol: SymbolId) -> u16 {
        self.frame_layout(function_symbol).arg_bytes
    }

    /// Returns the frame-relative offset assigned to a function-local temp.
    fn temp_offset(&self, function_symbol: SymbolId, temp: usize) -> u16 {
        self.layout.temp_offsets[&(function_symbol, temp)]
    }

    /// Returns the declared type of a symbol from the typed program.
    fn symbol_type(&self, symbol: SymbolId) -> Type {
        self.typed_program.symbols[symbol].ty
    }

    /// Returns the source-level name associated with a symbol id.
    fn symbol_name(&self, symbol: SymbolId) -> &str {
        &self.typed_program.symbols[symbol].name
    }

    /// Looks up one emitted Phase 17 dispatcher group by normalized function-pointer signature.
    fn function_pointer_dispatch_group(
        &self,
        signature: Type,
    ) -> Option<&FunctionPointerDispatchGroup> {
        let signature = signature.without_object_qualifiers();
        self.typed_program
            .function_pointer_groups
            .iter()
            .find(|group| group.ty == signature)
    }

    /// Expands one inline Phase 17 function-pointer signature into ordinary parameter types.
    fn function_pointer_param_types(&self, signature: Type) -> Vec<Type> {
        let mut params = Vec::new();
        for index in 0..signature.function_param_len().unwrap_or(0) {
            let scalar = signature
                .function_param_scalar(index)
                .expect("function pointer parameter index in range");
            params.push(Type::new(scalar));
        }
        params
    }

    /// Returns the byte width of one function-pointer argument list under the existing ABI.
    fn function_pointer_arg_bytes(&self, signature: Type) -> u16 {
        self.function_pointer_param_types(signature)
            .iter()
            .map(|ty| ty.byte_width() as u16)
            .sum()
    }

    /// Emits one per-signature dispatcher chain for every address-taken function group.
    fn emit_function_pointer_dispatchers(&mut self) {
        let groups = self.typed_program.function_pointer_groups.clone();
        for group in groups {
            let label = function_pointer_dispatch_label(group.ty);
            let miss_label = format!("{label}__miss");
            let done_label = format!("{label}__done");

            self.program.push(AsmLine::Label(label.clone()));
            self.program.push(AsmLine::Comment(format!(
                "phase17 function-pointer dispatcher for {}",
                group.ty
            )));
            self.select_bank(self.layout.helpers.scratch1);
            self.program.push(AsmLine::Instr(AsmInstr::Movf {
                f: low7(self.layout.helpers.scratch1),
                d: Dest::W,
            }));
            self.branch_if_bit_clear(low7(STATUS_ADDR), STATUS_Z_BIT, &miss_label);

            for target in &group.targets {
                let next_label = format!("{label}__next_{}", target.id);
                let case_label = format!(
                    "{label}__id_{}_{}",
                    target.id,
                    self.symbol_name(target.function)
                );
                self.load_addr_to_w(self.layout.helpers.scratch0);
                self.program
                    .push(AsmLine::Instr(AsmInstr::Xorlw(target.id as u8)));
                self.branch_if_bit_clear(low7(STATUS_ADDR), STATUS_Z_BIT, &next_label);
                self.program.push(AsmLine::Label(case_label));
                let callee_label = function_label(self.symbol_name(target.function));
                self.program.push(AsmLine::Comment(format!(
                    "dispatch id {} -> {}",
                    target.id,
                    self.symbol_name(target.function)
                )));
                self.program
                    .push(AsmLine::Instr(AsmInstr::SetPage(callee_label.clone())));
                self.program
                    .push(AsmLine::Instr(AsmInstr::Call(callee_label)));
                self.restore_code_page_after_call();
                self.jump_to_label(&done_label);
                self.program.push(AsmLine::Label(next_label));
            }

            self.program.push(AsmLine::Label(miss_label));
            self.emit_const_to_w(0);
            self.store_w_to_addr(self.layout.helpers.return_high);
            self.emit_const_to_w(0);
            self.store_w_to_addr(self.layout.helpers.return_upper0);
            self.emit_const_to_w(0);
            self.store_w_to_addr(self.layout.helpers.return_upper1);
            self.emit_const_to_w(0);
            self.program.push(AsmLine::Label(done_label));
            self.program.push(AsmLine::Instr(AsmInstr::Return));
        }
    }

    /// Creates a fresh backend-internal label name.
    fn unique_label(&mut self, prefix: &str) -> String {
        let label = format!("__{prefix}_{}", self.label_counter);
        self.label_counter += 1;
        label
    }
}

/// Builds the assembly label used for a function entrypoint.
fn function_label(name: &str) -> String {
    format!("fn_{name}")
}

/// Builds the assembly label used for one Phase 17 function-pointer dispatcher.
fn function_pointer_dispatch_label(signature: Type) -> String {
    let mut key = String::new();
    key.push_str(
        match signature
            .function_return_scalar()
            .unwrap_or(ScalarType::Void)
        {
            ScalarType::Void => "v",
            ScalarType::I8 => "i8",
            ScalarType::U8 => "u8",
            ScalarType::I16 => "i16",
            ScalarType::U16 => "u16",
            ScalarType::I32 => "i32",
            ScalarType::U32 => "u32",
            ScalarType::F32 => "f32",
            ScalarType::Q8_8 => "q8_8",
            ScalarType::UQ8_8 => "uq8_8",
            ScalarType::Q16_16 => "q16_16",
            ScalarType::UQ16_16 => "uq16_16",
        },
    );
    key.push_str("__");
    if signature.function_param_len().unwrap_or(0) == 0 {
        key.push_str("void");
    } else {
        for index in 0..signature.function_param_len().unwrap_or(0) {
            if index != 0 {
                key.push('_');
            }
            key.push_str(
                match signature
                    .function_param_scalar(index)
                    .unwrap_or(ScalarType::Void)
                {
                    ScalarType::Void => "v",
                    ScalarType::I8 => "i8",
                    ScalarType::U8 => "u8",
                    ScalarType::I16 => "i16",
                    ScalarType::U16 => "u16",
                    ScalarType::I32 => "i32",
                    ScalarType::U32 => "u32",
                    ScalarType::F32 => "f32",
                    ScalarType::Q8_8 => "q8_8",
                    ScalarType::UQ8_8 => "uq8_8",
                    ScalarType::Q16_16 => "q16_16",
                    ScalarType::UQ16_16 => "uq16_16",
                },
            );
        }
    }
    format!("__fp_dispatch_{key}")
}

/// Builds the assembly label used for one function-local basic block.
fn block_label(function_name: &str, block: usize) -> String {
    format!("fn_{function_name}_b{block}")
}

/// Computes the worst-case software-stack depth across normal code plus one interrupt frame.
fn compute_max_stack_depth_with_interrupts(
    typed_program: &TypedProgram,
    ir_program: &IrProgram,
    frames: &BTreeMap<SymbolId, FrameLayout>,
) -> u16 {
    let mut calls = BTreeMap::<SymbolId, Vec<SymbolId>>::new();
    let mut helper_depths = BTreeMap::<SymbolId, u16>::new();
    for function in &ir_program.functions {
        if function.is_interrupt {
            continue;
        }
        let mut callees = Vec::new();
        let mut helper_depth = 0u16;
        for block in &function.blocks {
            for instr in &block.instructions {
                match instr {
                    IrInstr::Call {
                        function: callee, ..
                    } => {
                        let callee_name = typed_program
                            .symbols
                            .get(*callee)
                            .map(|symbol| symbol.name.as_str())
                            .unwrap_or("");
                        if let Some(helper) = runtime_helper_for_math_call(callee_name) {
                            helper_depth = helper_depth
                                .max(runtime_helper_stack_cost(helper, Default::default()));
                        } else {
                            callees.push(*callee);
                        }
                    }
                    IrInstr::IndirectCall { signature, .. } => {
                        if let Some(group) = typed_program
                            .function_pointer_groups
                            .iter()
                            .find(|group| group.ty == signature.without_object_qualifiers())
                        {
                            callees.extend(group.targets.iter().map(|target| target.function));
                        }
                    }
                    IrInstr::Binary { dst, op, .. } => {
                        if let Some(helper) = binary_helper(*op, function.temp_types[*dst]) {
                            helper_depth = helper_depth
                                .max(runtime_helper_stack_cost(helper, Default::default()));
                        }
                    }
                    IrInstr::Cast { kind, .. } => {
                        if let Some(helper) = runtime_helper_for_cast(*kind) {
                            helper_depth = helper_depth
                                .max(runtime_helper_stack_cost(helper, Default::default()));
                        }
                    }
                    _ => {}
                }
            }
            if let IrTerminator::Branch {
                condition: IrCondition::Compare { op, ty, .. },
                ..
            } = &block.terminator
                && let Some(helper) = runtime_helper_for_float_compare(*op, *ty)
            {
                helper_depth =
                    helper_depth.max(runtime_helper_stack_cost(helper, Default::default()));
            }
        }
        calls.insert(function.symbol, callees);
        helper_depths.insert(function.symbol, helper_depth);
    }

    let mut memo = BTreeMap::new();
    let normal_max = calls
        .keys()
        .copied()
        .map(|symbol| {
            compute_function_stack_depth(
                symbol,
                &calls,
                &helper_depths,
                frames,
                &mut memo,
                &mut BTreeSet::new(),
            )
        })
        .max()
        .unwrap_or(0);
    let interrupt_extra = ir_program
        .functions
        .iter()
        .filter(|function| function.is_interrupt)
        .map(|function| {
            frames
                .get(&function.symbol)
                .map_or(0, |frame| frame.frame_bytes)
        })
        .max()
        .unwrap_or(0);
    normal_max + interrupt_extra
}

/// Builds one stack analysis model used by Phase 18 reports and visibility features.
fn analyze_stack(
    typed_program: &TypedProgram,
    ir_program: &IrProgram,
    layout: &StorageLayout,
    options: BackendOptions,
) -> StackAnalysis {
    let mut helper_depths = BTreeMap::<SymbolId, u16>::new();
    let mut direct_callees = BTreeMap::<SymbolId, Vec<SymbolId>>::new();
    let mut indirect_groups = BTreeMap::<SymbolId, Vec<IndirectTargetSet>>::new();
    let mut unknown_function_pointer_target_sets = 0u16;

    for function in &ir_program.functions {
        let mut callees = Vec::new();
        let mut helper_depth = 0u16;
        let mut fnptr_groups = Vec::new();
        for block in &function.blocks {
            for instr in &block.instructions {
                match instr {
                    IrInstr::Call {
                        function: callee, ..
                    } => {
                        let callee_name = typed_program
                            .symbols
                            .get(*callee)
                            .map(|symbol| symbol.name.as_str())
                            .unwrap_or("");
                        if let Some(helper) = runtime_helper_for_math_call(callee_name) {
                            helper_depth =
                                helper_depth.max(runtime_helper_stack_cost_for_profiles(
                                    helper,
                                    options.runtime_profile,
                                    options.math_profile,
                                ));
                        } else {
                            callees.push(*callee);
                        }
                    }
                    IrInstr::IndirectCall { signature, .. } => {
                        let target_set = if let Some(group) = typed_program
                            .function_pointer_groups
                            .iter()
                            .find(|group| group.ty == signature.without_object_qualifiers())
                        {
                            let targets = group
                                .targets
                                .iter()
                                .map(|target| target.function)
                                .collect::<Vec<_>>();
                            callees.extend(targets.iter().copied());
                            IndirectTargetSet {
                                signature: *signature,
                                targets,
                                unknown: false,
                            }
                        } else {
                            unknown_function_pointer_target_sets =
                                unknown_function_pointer_target_sets.saturating_add(1);
                            IndirectTargetSet {
                                signature: *signature,
                                targets: Vec::new(),
                                unknown: true,
                            }
                        };
                        fnptr_groups.push(target_set);
                    }
                    IrInstr::Binary { dst, op, .. } => {
                        if let Some(helper) = binary_helper(*op, function.temp_types[*dst]) {
                            helper_depth =
                                helper_depth.max(runtime_helper_stack_cost_for_profiles(
                                    helper,
                                    options.runtime_profile,
                                    options.math_profile,
                                ));
                        }
                    }
                    IrInstr::Cast { kind, .. } => {
                        if let Some(helper) = runtime_helper_for_cast(*kind) {
                            helper_depth =
                                helper_depth.max(runtime_helper_stack_cost_for_profiles(
                                    helper,
                                    options.runtime_profile,
                                    options.math_profile,
                                ));
                        }
                    }
                    _ => {}
                }
            }
            if let IrTerminator::Branch {
                condition: IrCondition::Compare { op, ty, .. },
                ..
            } = &block.terminator
                && let Some(helper) = runtime_helper_for_float_compare(*op, *ty)
            {
                helper_depth = helper_depth.max(runtime_helper_stack_cost_for_profiles(
                    helper,
                    options.runtime_profile,
                    options.math_profile,
                ));
            }
        }
        helper_depths.insert(function.symbol, helper_depth);
        direct_callees.insert(function.symbol, callees);
        indirect_groups.insert(function.symbol, fnptr_groups);
    }

    let mut memo_bytes = BTreeMap::new();
    let mut memo_depth = BTreeMap::new();
    let mut functions = Vec::new();
    for function in &typed_program.functions {
        functions.push(StackFunctionReport {
            symbol: function.symbol,
            frame: layout.frames[&function.symbol].clone(),
            helper_extra: helper_depths.get(&function.symbol).copied().unwrap_or(0),
            max_stack_bytes: stack_bytes_for_function(
                function.symbol,
                &direct_callees,
                &helper_depths,
                &layout.frames,
                &mut memo_bytes,
                &mut BTreeSet::new(),
            ),
            max_call_depth: call_depth_for_function(
                function.symbol,
                &direct_callees,
                &mut memo_depth,
                &mut BTreeSet::new(),
            ),
            direct_callees: direct_callees
                .get(&function.symbol)
                .cloned()
                .unwrap_or_default(),
            indirect_groups: indirect_groups
                .get(&function.symbol)
                .cloned()
                .unwrap_or_default(),
        });
    }
    functions.sort_by_key(|info| typed_program.symbols[info.symbol].name.clone());

    let isr_frame_bytes = ir_program
        .functions
        .iter()
        .filter(|function| function.is_interrupt)
        .map(|function| {
            layout
                .frames
                .get(&function.symbol)
                .map_or(0, |frame| frame.frame_bytes)
        })
        .max()
        .unwrap_or(0);
    let isr_context_bytes = layout.interrupt.map_or(0, |_| {
        if layout.helpers.return_upper0 == layout.helpers.return_high {
            13
        } else {
            15
        }
    });
    let function_pointer_groups = typed_program.function_pointer_groups.len() as u16;
    let function_pointer_targets = typed_program
        .function_pointer_groups
        .iter()
        .map(|group| group.targets.len() as u16)
        .sum();

    StackAnalysis {
        functions,
        summary: StackReportSummary {
            stack_base: layout.stack_base,
            stack_limit: layout.stack_limit,
            stack_capacity: layout.stack_capacity,
            static_max_stack: layout.max_stack_depth,
            max_call_depth: memo_depth.values().copied().max().unwrap_or(0),
            isr_frame_bytes,
            isr_context_bytes,
            function_pointer_groups,
            function_pointer_targets,
            unknown_function_pointer_target_sets,
            stack_check: options.stack_check,
        },
    }
}

#[cfg(test)]
/// Compatibility wrapper for tests that do not model interrupts explicitly.
fn compute_max_stack_depth(
    ir_program: &IrProgram,
    frames: &BTreeMap<SymbolId, FrameLayout>,
) -> u16 {
    compute_max_stack_depth_with_interrupts(
        &TypedProgram {
            symbols: Vec::new(),
            globals: Vec::new(),
            functions: Vec::new(),
            function_pointer_groups: Vec::new(),
        },
        ir_program,
        frames,
    )
}

/// Computes the worst-case stack usage while one function is active.
fn compute_function_stack_depth(
    symbol: SymbolId,
    calls: &BTreeMap<SymbolId, Vec<SymbolId>>,
    helper_depths: &BTreeMap<SymbolId, u16>,
    frames: &BTreeMap<SymbolId, FrameLayout>,
    memo: &mut BTreeMap<SymbolId, u16>,
    active: &mut BTreeSet<SymbolId>,
) -> u16 {
    if let Some(depth) = memo.get(&symbol).copied() {
        return depth;
    }
    if !active.insert(symbol) {
        return frames.get(&symbol).map_or(0, |frame| frame.frame_bytes);
    }

    let own = frames.get(&symbol).map_or(0, |frame| frame.frame_bytes);
    let nested = calls
        .get(&symbol)
        .into_iter()
        .flatten()
        .map(|callee| {
            let arg_bytes = frames.get(callee).map_or(0, |frame| frame.arg_bytes);
            arg_bytes
                + compute_function_stack_depth(*callee, calls, helper_depths, frames, memo, active)
        })
        .max()
        .unwrap_or(0);
    let depth = own + nested.max(helper_depths.get(&symbol).copied().unwrap_or(0));
    active.remove(&symbol);
    memo.insert(symbol, depth);
    depth
}

/// Computes worst-case stack bytes while one function is active.
fn stack_bytes_for_function(
    symbol: SymbolId,
    calls: &BTreeMap<SymbolId, Vec<SymbolId>>,
    helper_depths: &BTreeMap<SymbolId, u16>,
    frames: &BTreeMap<SymbolId, FrameLayout>,
    memo: &mut BTreeMap<SymbolId, u16>,
    active: &mut BTreeSet<SymbolId>,
) -> u16 {
    if let Some(depth) = memo.get(&symbol).copied() {
        return depth;
    }
    if !active.insert(symbol) {
        return frames.get(&symbol).map_or(0, |frame| frame.frame_bytes);
    }

    let own = frames.get(&symbol).map_or(0, |frame| frame.frame_bytes);
    let nested = calls
        .get(&symbol)
        .into_iter()
        .flatten()
        .map(|callee| {
            let arg_bytes = frames.get(callee).map_or(0, |frame| frame.arg_bytes);
            arg_bytes
                + stack_bytes_for_function(*callee, calls, helper_depths, frames, memo, active)
        })
        .max()
        .unwrap_or(0);
    let depth = own + nested.max(helper_depths.get(&symbol).copied().unwrap_or(0));
    active.remove(&symbol);
    memo.insert(symbol, depth);
    depth
}

/// Computes one call-depth count for reporting.
fn call_depth_for_function(
    symbol: SymbolId,
    calls: &BTreeMap<SymbolId, Vec<SymbolId>>,
    memo: &mut BTreeMap<SymbolId, u16>,
    active: &mut BTreeSet<SymbolId>,
) -> u16 {
    if let Some(depth) = memo.get(&symbol).copied() {
        return depth;
    }
    if !active.insert(symbol) {
        return 1;
    }
    let nested = calls
        .get(&symbol)
        .into_iter()
        .flatten()
        .map(|callee| call_depth_for_function(*callee, calls, memo, active))
        .max()
        .unwrap_or(0);
    let depth = 1 + nested;
    active.remove(&symbol);
    memo.insert(symbol, depth);
    depth
}

fn runtime_helper_stack_cost(helper: RuntimeHelper, profile: RuntimeProfile) -> u16 {
    runtime_helper_stack_cost_for_profiles(helper, profile, MathProfile::Balanced)
}

fn runtime_helper_stack_cost_for_profiles(
    helper: RuntimeHelper,
    runtime_profile: RuntimeProfile,
    math_profile: MathProfile,
) -> u16 {
    let info = helper.info();
    let dependency_cost = helper
        .dependencies_for_profiles(runtime_profile, math_profile)
        .iter()
        .map(|dependency| {
            runtime_helper_stack_cost_for_profiles(*dependency, runtime_profile, math_profile)
        })
        .max()
        .unwrap_or(0);
    info.arg_bytes
        .saturating_add(info.frame_bytes)
        .saturating_add(dependency_cost)
}

fn runtime_helper_variant(
    helper: RuntimeHelper,
    runtime_profile: RuntimeProfile,
    math_profile: MathProfile,
) -> &'static str {
    if helper == RuntimeHelper::F32Sqrt {
        return match math_profile {
            MathProfile::Compact | MathProfile::Balanced => "compact_approx",
            MathProfile::Precise => "precise_table_refined",
        };
    }
    if helper
        .dependencies_for_profiles(runtime_profile, math_profile)
        .is_empty()
    {
        "balanced"
    } else {
        runtime_profile.as_str()
    }
}

struct ResourceReportInputs<'a> {
    target: &'a TargetDevice,
    typed_program: &'a TypedProgram,
    layout: &'a StorageLayout,
    words: &'a BTreeMap<u16, u16>,
    labels: &'a BTreeMap<String, u16>,
    stack: &'a StackReportSummary,
    relaxation: LinkerRelaxationStats,
    runtime_profile: RuntimeProfile,
    math_profile: MathProfile,
}

/// Builds deterministic target resource usage data for `--size`, maps, and reports.
fn build_resource_report(inputs: ResourceReportInputs<'_>) -> ResourceReport {
    let ResourceReportInputs {
        target,
        typed_program,
        layout,
        words,
        labels,
        stack,
        relaxation,
        runtime_profile,
        math_profile,
    } = inputs;
    let contributions =
        collect_resource_contributions(target, typed_program, layout, words, labels);
    let page_layout = build_page_layout(target, words);
    let code_sections = build_code_sections(&contributions);
    let program_words_used = u16::try_from(
        words
            .keys()
            .filter(|addr| target.program_memory.contains(**addr))
            .count(),
    )
    .unwrap_or(u16::MAX);
    let highest_program_word = words
        .keys()
        .copied()
        .filter(|addr| target.program_memory.contains(*addr))
        .max()
        .unwrap_or(target.program_memory.start);
    let static_data_bytes = static_data_bytes(typed_program, layout);
    let abi_slot_bytes = u16::try_from(abi_slot_addresses(layout).len()).unwrap_or(u16::MAX);
    let isr_context_bytes = u16::try_from(interrupt_context_addresses(layout).len()).unwrap_or(0);
    let data_ram_used = static_data_bytes
        .saturating_add(abi_slot_bytes)
        .saturating_add(isr_context_bytes)
        .saturating_add(layout.stack_capacity);
    let rom_table_words = contributions
        .iter()
        .filter(|item| item.kind == ResourceContributionKind::RomTable)
        .map(|item| item.words)
        .sum();
    let helpers_included = contributions
        .iter()
        .filter(|item| {
            matches!(
                item.kind,
                ResourceContributionKind::RuntimeHelper
                    | ResourceContributionKind::FixedPointHelper
                    | ResourceContributionKind::FloatHelper
                    | ResourceContributionKind::MathHelper
                    | ResourceContributionKind::ShiftHelper
            )
        })
        .count();
    let runtime_helper_words = contributions
        .iter()
        .filter(|item| is_runtime_helper_contribution(item))
        .map(|item| item.words)
        .sum::<u16>();
    let integer_helper_words =
        helper_words_by_category(&contributions, RuntimeHelperCategory::Integer);
    let fixed_helper_words = helper_words_by_category(&contributions, RuntimeHelperCategory::Fixed);
    let float_helper_words = helper_words_by_category(&contributions, RuntimeHelperCategory::Float);
    let math_helper_words = helper_words_by_category(&contributions, RuntimeHelperCategory::Math);
    let conversion_helper_words =
        helper_words_by_category(&contributions, RuntimeHelperCategory::Conversion);
    let shift_helper_words = helper_words_by_category(&contributions, RuntimeHelperCategory::Shift);
    let division_helper_words =
        helper_words_by_category(&contributions, RuntimeHelperCategory::Division);
    let function_pointer_dispatchers = contributions
        .iter()
        .filter(|item| item.kind == ResourceContributionKind::FunctionPointerDispatcher)
        .count();
    let dispatcher_words = contributions
        .iter()
        .filter(|item| item.kind == ResourceContributionKind::FunctionPointerDispatcher)
        .map(|item| item.words)
        .sum::<u16>();

    let summary = ResourceSummary {
        program_words_used,
        program_words_available: target.program_words,
        highest_program_word,
        data_ram_used,
        modeled_data_ram_available: target.modeled_data_ram_bytes(),
        device_data_ram_available: target.data_ram_bytes,
        static_data_bytes,
        abi_slot_bytes,
        isr_context_bytes,
        stack_capacity: layout.stack_capacity,
        estimated_max_stack: layout.max_stack_depth,
        rom_table_words,
        helpers_included: u16::try_from(helpers_included).unwrap_or(u16::MAX),
        runtime_helper_words,
        integer_helper_words,
        fixed_helper_words,
        float_helper_words,
        math_helper_words,
        conversion_helper_words,
        shift_helper_words,
        division_helper_words,
        dispatcher_words,
        function_pointer_dispatchers: u16::try_from(function_pointer_dispatchers)
            .unwrap_or(u16::MAX),
        unknown_function_pointer_target_sets: stack.unknown_function_pointer_target_sets,
        page_setup_removed: u16::try_from(relaxation.removed_redundant_setpages)
            .unwrap_or(u16::MAX),
        page_relaxation_passes: u16::try_from(relaxation.passes).unwrap_or(u16::MAX),
        runtime_profile,
        math_profile,
    };

    ResourceReport {
        summary,
        size_text: render_size_summary(target, &summary, &page_layout),
        text: render_memory_report(
            target,
            layout,
            stack,
            &summary,
            &contributions,
            &page_layout,
        ),
        map_lines: render_resource_map_lines(target, &summary, &page_layout),
        page_layout,
        code_sections,
    }
}

/// Validates target fit after all code, helpers, dispatchers, and ROM tables are placed.
fn validate_resource_fit(
    target: &TargetDevice,
    report: &ResourceReport,
    words: &BTreeMap<u16, u16>,
    options: BackendOptions,
    diagnostics: &mut DiagnosticBag,
) {
    if target.program_memory.size() != target.program_words {
        diagnostics.error(
            "backend",
            None,
            format!(
                "memory descriptor for target {} has inconsistent program range",
                target.name
            ),
            Some("program_memory range size must match program_words".to_string()),
        );
    }
    if !target.program_memory.contains(target.vectors.reset)
        || !target.program_memory.contains(target.vectors.interrupt)
    {
        diagnostics.error(
            "backend",
            None,
            format!(
                "memory descriptor for target {} has vector outside program memory",
                target.name
            ),
            None,
        );
    }
    if target.allocatable_gpr.is_empty() {
        diagnostics.error(
            "backend",
            None,
            format!(
                "memory descriptor for target {} lacks allocatable RAM ranges",
                target.name
            ),
            None,
        );
    }

    if options.enforce_resource_limits
        && let Some(highest) = words
            .keys()
            .copied()
            .filter(|addr| *addr != target.vectors.config_word)
            .max()
        && !target.program_memory.contains(highest)
    {
        diagnostics.error(
            "backend",
            None,
            format!("program memory overflow for target {}", target.name),
            Some(format!(
                "highest used word: 0x{highest:04X}; available range: 0x{:04X}..0x{:04X}",
                target.program_memory.start, target.program_memory.end
            )),
        );
    }

    if options.enforce_resource_limits && words.contains_key(&target.vectors.config_word) {
        diagnostics.error(
            "backend",
            None,
            format!(
                "program code overlaps config word 0x{:04X} for target {}",
                target.vectors.config_word, target.name
            ),
            Some(
                "move code/ROM tables below normal program memory; config is emitted separately"
                    .to_string(),
            ),
        );
    }

    if report.summary.data_ram_used > report.summary.modeled_data_ram_available {
        diagnostics.error(
            "backend",
            None,
            format!("data RAM overflow for target {}", target.name),
            Some(format!(
                "reserved {} byte(s), modeled GPR capacity {} byte(s)",
                report.summary.data_ram_used, report.summary.modeled_data_ram_available
            )),
        );
    }

    if report.summary.estimated_max_stack > report.summary.stack_capacity {
        diagnostics.error(
            "backend",
            None,
            format!("stack region overflow for target {}", target.name),
            Some(format!(
                "estimated max stack {} byte(s), stack region {} byte(s)",
                report.summary.estimated_max_stack, report.summary.stack_capacity
            )),
        );
    }

    emit_runtime_budget_warning(target, report, options, diagnostics);
    emit_sqrtf_math_profile_warning(report, options, diagnostics);
}

fn emit_runtime_budget_warning(
    target: &TargetDevice,
    report: &ResourceReport,
    options: BackendOptions,
    diagnostics: &mut DiagnosticBag,
) {
    if report.summary.runtime_helper_words == 0 {
        return;
    }
    let percent = (u32::from(report.summary.runtime_helper_words) * 100)
        / u32::from(report.summary.program_words_available.max(1));
    let threshold = match options.runtime_profile {
        RuntimeProfile::Small => 30,
        RuntimeProfile::Balanced | RuntimeProfile::Fast => 75,
    };
    if percent < threshold {
        return;
    }
    // Advisory resource-budget warnings intentionally do not participate in
    // `-Werror`; resource overflow remains a hard error separately.
    diagnostics.diagnostics.push(Diagnostic {
        severity: Severity::Warning,
        stage: "backend",
        message: format!(
            "runtime helpers use {} words on {} ({}% of program memory)",
            report.summary.runtime_helper_words, target.name, percent
        ),
        span: None,
        note: Some(format!(
            "top helper categories: float={} math={} fixed={} conversion={} division={}",
            report.summary.float_helper_words,
            report.summary.math_helper_words,
            report.summary.fixed_helper_words,
            report.summary.conversion_helper_words,
            report.summary.division_helper_words
        )),
        suggestion: Some(
            "inspect --memory-report; consider fixed-point, --runtime-profile small, or a larger target"
                .to_string(),
        ),
        code: Some("runtime-helper-budget"),
    });
}

fn emit_sqrtf_math_profile_warning(
    report: &ResourceReport,
    options: BackendOptions,
    diagnostics: &mut DiagnosticBag,
) {
    if !options.enforce_resource_limits
        || !matches!(
            options.math_profile,
            MathProfile::Compact | MathProfile::Balanced
        )
        || !report
            .code_sections
            .iter()
            .any(|section| section.name == RuntimeHelper::F32Sqrt.label())
    {
        return;
    }
    diagnostics.diagnostics.push(Diagnostic {
        severity: Severity::Warning,
        stage: "backend",
        message: format!(
            "sqrtf uses `{}` math profile compact approximation",
            options.math_profile.as_str()
        ),
        span: None,
        note: Some(
            "dynamic sqrtf is finite-only, exact for documented simple values, and approximate for fallback positive inputs; constant sqrtf folding uses compile-time f32 sqrt"
                .to_string(),
        ),
        suggestion: Some(
            "use `--math-profile precise` for the larger refined helper, then inspect --size/--memory-report for target fit"
                .to_string(),
        ),
        code: Some("sqrtf-math-profile"),
    });
}

fn render_size_summary(
    target: &TargetDevice,
    summary: &ResourceSummary,
    pages: &[PageLayoutSummary; 4],
) -> String {
    let mut output = String::new();
    let _ = writeln!(output, "Target: {}", target.name.to_ascii_uppercase());
    let _ = writeln!(
        output,
        "Runtime profile: {}",
        summary.runtime_profile.as_str()
    );
    let _ = writeln!(output, "Math profile: {}", summary.math_profile.as_str());
    let _ = writeln!(
        output,
        "Program words: {} / {}",
        summary.program_words_used, summary.program_words_available
    );
    let _ = writeln!(
        output,
        "Data RAM: {} / {} bytes modeled (device total: {} bytes)",
        summary.data_ram_used,
        summary.modeled_data_ram_available,
        summary.device_data_ram_available
    );
    let _ = writeln!(output, "Static data: {} bytes", summary.static_data_bytes);
    let _ = writeln!(
        output,
        "Software stack region: {} bytes",
        summary.stack_capacity
    );
    let _ = writeln!(
        output,
        "Estimated max stack: {} bytes",
        summary.estimated_max_stack
    );
    let _ = writeln!(output, "ROM table words: {}", summary.rom_table_words);
    let _ = writeln!(output, "Helpers included: {}", summary.helpers_included);
    let _ = writeln!(
        output,
        "Runtime helpers: {} words",
        summary.runtime_helper_words
    );
    let _ = writeln!(
        output,
        "  integer: {}  division: {}  fixed: {}  float: {}  math: {}  conversion: {}  shift: {}  dispatchers: {}",
        summary.integer_helper_words,
        summary.division_helper_words,
        summary.fixed_helper_words,
        summary.float_helper_words,
        summary.math_helper_words,
        summary.conversion_helper_words,
        summary.shift_helper_words,
        summary.dispatcher_words
    );
    let page_text = pages
        .iter()
        .filter(|page| page.used_words != 0)
        .map(|page| format!("page {}: {} used", page.page, page.used_words))
        .collect::<Vec<_>>()
        .join(", ");
    let _ = writeln!(
        output,
        "Page layout: {}",
        if page_text.is_empty() {
            "empty".to_string()
        } else {
            page_text
        }
    );
    let _ = writeln!(
        output,
        "Page setup relaxation: removed {} setpage(s) in {} pass(es)",
        summary.page_setup_removed, summary.page_relaxation_passes
    );
    output
}

fn render_memory_report(
    target: &TargetDevice,
    layout: &StorageLayout,
    stack: &StackReportSummary,
    summary: &ResourceSummary,
    contributions: &[ResourceContribution],
    pages: &[PageLayoutSummary; 4],
) -> String {
    let mut output = String::new();
    let _ = writeln!(output, "Memory report");
    let _ = writeln!(output, "-------------");
    let _ = writeln!(output, "target: {}", target.name);
    let _ = writeln!(
        output,
        "runtime profile: {}",
        summary.runtime_profile.as_str()
    );
    let _ = writeln!(output, "math profile: {}", summary.math_profile.as_str());
    let _ = writeln!(
        output,
        "program range: 0x{:04X}..0x{:04X} ({} words)",
        target.program_memory.start, target.program_memory.end, target.program_words
    );
    let _ = writeln!(
        output,
        "vectors: reset=0x{:04X} interrupt=0x{:04X} config=0x{:04X}",
        target.vectors.reset, target.vectors.interrupt, target.vectors.config_word
    );
    let _ = writeln!(
        output,
        "program words: {} / {} highest=0x{:04X}",
        summary.program_words_used, summary.program_words_available, summary.highest_program_word
    );
    let _ = writeln!(
        output,
        "data RAM: {} / {} bytes modeled (device total: {} bytes)",
        summary.data_ram_used,
        summary.modeled_data_ram_available,
        summary.device_data_ram_available
    );
    let _ = writeln!(
        output,
        "data breakdown: static={} abi/runtime={} isr_context={} stack_region={}",
        summary.static_data_bytes,
        summary.abi_slot_bytes,
        summary.isr_context_bytes,
        summary.stack_capacity
    );
    let _ = writeln!(
        output,
        "stack: base=0x{:04X} limit=0x{:04X} capacity={} estimated_max={} check={}",
        layout.stack_base,
        layout.stack_limit,
        layout.stack_capacity,
        summary.estimated_max_stack,
        if stack.stack_check { "on" } else { "off" }
    );
    let _ = writeln!(
        output,
        "function pointers: groups={} targets={} unknown_target_sets={}",
        stack.function_pointer_groups,
        stack.function_pointer_targets,
        stack.unknown_function_pointer_target_sets
    );
    let _ = writeln!(
        output,
        "page setup relaxation: removed_setpages={} passes={}",
        summary.page_setup_removed, summary.page_relaxation_passes
    );
    let _ = writeln!(
        output,
        "runtime helpers: total={} integer={} division={} fixed={} float={} math={} conversion={} shift={} dispatchers={}",
        summary.runtime_helper_words,
        summary.integer_helper_words,
        summary.division_helper_words,
        summary.fixed_helper_words,
        summary.float_helper_words,
        summary.math_helper_words,
        summary.conversion_helper_words,
        summary.shift_helper_words,
        summary.dispatcher_words
    );
    let _ = writeln!(output);
    render_memory_ranges(&mut output, "allocatable GPR", target.allocatable_gpr);
    render_memory_ranges(&mut output, "shared GPR", target.shared_gpr);
    render_memory_ranges(&mut output, "reserved/SFR RAM", target.reserved_ram);
    let _ = writeln!(
        output,
        "ROM table region: 0x{:04X}..0x{:04X}",
        target.rom_table_region.start, target.rom_table_region.end
    );
    let _ = writeln!(
        output,
        "default stack candidate: 0x{:04X}..0x{:04X}",
        target.default_stack_region.start, target.default_stack_region.end
    );

    let _ = writeln!(output);
    let _ = writeln!(output, "Page layout");
    let _ = writeln!(output, "-----------");
    for page in pages {
        let _ = writeln!(
            output,
            "page {}: 0x{:04X}..0x{:04X} used={} free={}",
            page.page, page.start, page.end, page.used_words, page.free_words
        );
    }

    let _ = writeln!(output);
    let _ = writeln!(output, "Program sections");
    let _ = writeln!(output, "----------------");
    for item in contributions {
        let end = item.start.saturating_add(item.words.saturating_sub(1));
        let _ = writeln!(
            output,
            "0x{:04X}..0x{:04X}  page {}..{}  {:>4} words  {:<27} {}",
            item.start,
            end,
            control_page(item.start),
            control_page(end),
            item.words,
            item.kind,
            item.name
        );
    }

    let helpers = contributions
        .iter()
        .filter(|item| is_runtime_helper_contribution(item))
        .collect::<Vec<_>>();
    if !helpers.is_empty() {
        let _ = writeln!(output);
        let _ = writeln!(output, "Runtime helper contribution");
        let _ = writeln!(output, "Runtime Helper Contributors");
        let _ = writeln!(output, "---------------------------");
        for helper in helpers {
            let frame = helper.stack_frame.unwrap_or(0);
            if let Some(runtime_helper) = runtime_helper_by_label(&helper.name) {
                let catalog = runtime_helper.catalog_entry();
                let dependencies = runtime_helper
                    .dependencies_for_profiles(summary.runtime_profile, summary.math_profile);
                let dependencies = if dependencies.is_empty() {
                    "(none)".to_string()
                } else {
                    dependencies
                        .iter()
                        .map(|dependency| dependency.label())
                        .collect::<Vec<_>>()
                        .join(", ")
                };
                let _ = writeln!(
                    output,
                    "{}: category={} variant={} actual={} estimated={} args={} locals={} frame={} required_by={} deps={} constraints={}",
                    helper.name,
                    catalog.category.as_str(),
                    runtime_helper_variant(
                        runtime_helper,
                        summary.runtime_profile,
                        summary.math_profile
                    ),
                    helper.words,
                    catalog.estimated_words,
                    catalog.arg_bytes,
                    catalog.local_bytes,
                    frame,
                    catalog.required_by,
                    dependencies,
                    catalog.target_constraints
                );
            } else {
                let _ = writeln!(
                    output,
                    "{}: category=unknown actual={} frame={}",
                    helper.name, helper.words, frame
                );
            }
        }

        let _ = writeln!(output);
        let _ = writeln!(output, "Runtime Helper Dependency Graph");
        let _ = writeln!(output, "-------------------------------");
        for helper in contributions
            .iter()
            .filter_map(|item| runtime_helper_by_label(&item.name))
        {
            let dependencies =
                helper.dependencies_for_profiles(summary.runtime_profile, summary.math_profile);
            if dependencies.is_empty() {
                let _ = writeln!(output, "{} -> (none)", helper.label());
            } else {
                let joined = dependencies
                    .iter()
                    .map(|dependency| dependency.label())
                    .collect::<Vec<_>>()
                    .join(", ");
                let _ = writeln!(output, "{} -> {joined}", helper.label());
            }
        }
    }

    let mut largest = contributions.to_vec();
    largest.sort_by(|lhs, rhs| {
        rhs.words
            .cmp(&lhs.words)
            .then_with(|| lhs.name.cmp(&rhs.name))
    });
    let _ = writeln!(output);
    let _ = writeln!(output, "Largest contributors");
    let _ = writeln!(output, "--------------------");
    for item in largest.into_iter().take(8) {
        let _ = writeln!(
            output,
            "{}: {} words ({})",
            item.name, item.words, item.kind
        );
    }

    output
}

fn render_resource_map_lines(
    target: &TargetDevice,
    summary: &ResourceSummary,
    pages: &[PageLayoutSummary; 4],
) -> Vec<String> {
    let mut lines = vec![
        format!("Target: {}", target.name.to_ascii_uppercase()),
        format!("Runtime profile: {}", summary.runtime_profile.as_str()),
        format!("Math profile: {}", summary.math_profile.as_str()),
        format!(
            "Program words: {} / {}",
            summary.program_words_used, summary.program_words_available
        ),
        format!(
            "Data RAM: {} / {} bytes modeled (device total: {} bytes)",
            summary.data_ram_used,
            summary.modeled_data_ram_available,
            summary.device_data_ram_available
        ),
        format!("Static data: {} bytes", summary.static_data_bytes),
        format!("Software stack region: {} bytes", summary.stack_capacity),
        format!("Estimated max stack: {} bytes", summary.estimated_max_stack),
        format!("ROM table words: {}", summary.rom_table_words),
        format!("Helpers included: {}", summary.helpers_included),
        format!("Runtime helpers: {} words", summary.runtime_helper_words),
        format!(
            "Runtime helper words by category: integer={} division={} fixed={} float={} math={} conversion={} shift={} dispatchers={}",
            summary.integer_helper_words,
            summary.division_helper_words,
            summary.fixed_helper_words,
            summary.float_helper_words,
            summary.math_helper_words,
            summary.conversion_helper_words,
            summary.shift_helper_words,
            summary.dispatcher_words
        ),
        format!(
            "Function pointer dispatchers: {}",
            summary.function_pointer_dispatchers
        ),
        format!(
            "Unknown function pointer target sets: {}",
            summary.unknown_function_pointer_target_sets
        ),
        format!(
            "Page setup relaxation: removed {} setpage(s) in {} pass(es)",
            summary.page_setup_removed, summary.page_relaxation_passes
        ),
    ];
    for page in pages {
        lines.push(format!(
            "Page {} words: {} used / {} free (0x{:04X}..0x{:04X})",
            page.page, page.used_words, page.free_words, page.start, page.end
        ));
    }
    lines
}

fn render_memory_ranges(output: &mut String, title: &str, ranges: &[MemoryRange]) {
    if ranges.is_empty() {
        let _ = writeln!(output, "{title}: (none)");
        return;
    }
    let joined = ranges
        .iter()
        .map(|range| format!("0x{:04X}..0x{:04X}", range.start, range.end))
        .collect::<Vec<_>>()
        .join(", ");
    let bytes = ranges.iter().map(|range| range.size()).sum::<u16>();
    let _ = writeln!(output, "{title}: {joined} ({bytes} bytes)");
}

fn control_page(addr: u16) -> u8 {
    ((addr >> 11) & 0x03) as u8
}

fn page_range(page: u8) -> (u16, u16) {
    let start = u16::from(page) * 0x0800;
    (start, start + 0x07FF)
}

fn build_page_layout(target: &TargetDevice, words: &BTreeMap<u16, u16>) -> [PageLayoutSummary; 4] {
    let mut pages = [PageLayoutSummary::default(); 4];
    for page in 0u8..4 {
        let (start, end) = page_range(page);
        let available = (start..=end)
            .filter(|addr| target.program_memory.contains(*addr))
            .count() as u16;
        let used = words
            .keys()
            .filter(|addr| {
                **addr >= start && **addr <= end && target.program_memory.contains(**addr)
            })
            .count() as u16;
        pages[usize::from(page)] = PageLayoutSummary {
            page,
            start,
            end,
            used_words: used,
            free_words: available.saturating_sub(used),
        };
    }
    pages
}

fn build_code_sections(contributions: &[ResourceContribution]) -> Vec<CodeSectionSummary> {
    contributions
        .iter()
        .map(|item| {
            let end = item.start.saturating_add(item.words.saturating_sub(1));
            CodeSectionSummary {
                name: item.name.clone(),
                kind: item.kind.to_string(),
                start: item.start,
                end,
                words: item.words,
                page_start: control_page(item.start),
                page_end: control_page(end),
            }
        })
        .collect()
}

fn is_runtime_helper_contribution(item: &ResourceContribution) -> bool {
    matches!(
        item.kind,
        ResourceContributionKind::RuntimeHelper
            | ResourceContributionKind::FixedPointHelper
            | ResourceContributionKind::FloatHelper
            | ResourceContributionKind::MathHelper
            | ResourceContributionKind::ShiftHelper
    )
}

fn helper_words_by_category(
    contributions: &[ResourceContribution],
    category: RuntimeHelperCategory,
) -> u16 {
    contributions
        .iter()
        .filter_map(|item| {
            runtime_helper_by_label(&item.name)
                .filter(|helper| helper.category() == category)
                .map(|_| item.words)
        })
        .sum()
}

fn collect_resource_contributions(
    target: &TargetDevice,
    typed_program: &TypedProgram,
    layout: &StorageLayout,
    words: &BTreeMap<u16, u16>,
    labels: &BTreeMap<String, u16>,
) -> Vec<ResourceContribution> {
    let mut starts = Vec::<(u16, String, ResourceContributionKind, Option<u16>)>::new();
    push_label_if_present(
        &mut starts,
        labels,
        "__reset_vector",
        ResourceContributionKind::VectorStartup,
        None,
    );
    push_label_if_present(
        &mut starts,
        labels,
        "__interrupt_vector",
        ResourceContributionKind::VectorStartup,
        None,
    );
    push_label_if_present(
        &mut starts,
        labels,
        "__start",
        ResourceContributionKind::VectorStartup,
        None,
    );
    push_label_if_present(
        &mut starts,
        labels,
        "__stack_overflow_trap",
        ResourceContributionKind::Internal,
        None,
    );

    for function in &typed_program.functions {
        let name = &typed_program.symbols[function.symbol].name;
        let label = function_label(name);
        let frame = layout
            .frames
            .get(&function.symbol)
            .map(|frame| frame.frame_bytes);
        push_label_if_present(
            &mut starts,
            labels,
            &label,
            ResourceContributionKind::Function,
            frame,
        );
    }

    for group in &typed_program.function_pointer_groups {
        let label = function_pointer_dispatch_label(group.ty);
        push_label_if_present(
            &mut starts,
            labels,
            &label,
            ResourceContributionKind::FunctionPointerDispatcher,
            None,
        );
    }

    for helper in RuntimeHelper::ALL {
        let info = helper.info();
        push_label_if_present(
            &mut starts,
            labels,
            info.label,
            runtime_helper_resource_kind(*helper),
            Some(info.frame_bytes),
        );
    }

    for symbol in typed_program
        .symbols
        .iter()
        .filter(|symbol| symbol.ty.is_rom())
    {
        let label = rom_object_label(symbol.id);
        if let Some(addr) = labels.get(&label).copied() {
            starts.push((
                addr,
                format_rom_symbol_name(symbol),
                ResourceContributionKind::RomTable,
                None,
            ));
        }
    }

    starts.sort_by(|lhs, rhs| lhs.0.cmp(&rhs.0).then_with(|| lhs.1.cmp(&rhs.1)));
    starts.dedup_by(|lhs, rhs| lhs.0 == rhs.0 && lhs.1 == rhs.1);
    let program_end = words
        .keys()
        .copied()
        .filter(|addr| target.program_memory.contains(*addr))
        .max()
        .map(|addr| addr.saturating_add(1))
        .unwrap_or(target.program_memory.start);

    starts
        .iter()
        .enumerate()
        .map(|(index, (start, name, kind, frame))| {
            let end = starts
                .iter()
                .skip(index + 1)
                .map(|(addr, _, _, _)| *addr)
                .find(|addr| *addr > *start)
                .unwrap_or(program_end);
            ResourceContribution {
                name: name.clone(),
                kind: *kind,
                start: *start,
                words: end.saturating_sub(*start),
                stack_frame: *frame,
            }
        })
        .filter(|item| item.words > 0)
        .collect()
}

fn push_label_if_present(
    starts: &mut Vec<(u16, String, ResourceContributionKind, Option<u16>)>,
    labels: &BTreeMap<String, u16>,
    label: &str,
    kind: ResourceContributionKind,
    frame: Option<u16>,
) {
    if let Some(addr) = labels.get(label).copied() {
        starts.push((addr, label.to_string(), kind, frame));
    }
}

fn runtime_helper_resource_kind(helper: RuntimeHelper) -> ResourceContributionKind {
    match helper.category() {
        RuntimeHelperCategory::Fixed => ResourceContributionKind::FixedPointHelper,
        RuntimeHelperCategory::Math => ResourceContributionKind::MathHelper,
        RuntimeHelperCategory::Float | RuntimeHelperCategory::Conversion => {
            ResourceContributionKind::FloatHelper
        }
        RuntimeHelperCategory::Shift => ResourceContributionKind::ShiftHelper,
        RuntimeHelperCategory::Integer | RuntimeHelperCategory::Division => {
            ResourceContributionKind::RuntimeHelper
        }
    }
}

fn runtime_helper_for_cast(kind: CastKind) -> Option<RuntimeHelper> {
    match kind {
        CastKind::F32ToQ16 => Some(RuntimeHelper::F32ToQ16),
        CastKind::Q16ToF32 => Some(RuntimeHelper::Q16ToF32),
        CastKind::I32ToF32 => Some(RuntimeHelper::I32ToF32),
        CastKind::U32ToF32 => Some(RuntimeHelper::U32ToF32),
        CastKind::F32ToI32 => Some(RuntimeHelper::F32ToI32),
        CastKind::F32ToU32 => Some(RuntimeHelper::F32ToU32),
        CastKind::ZeroExtend | CastKind::SignExtend | CastKind::Truncate | CastKind::Bitcast => {
            None
        }
    }
}

fn runtime_helper_for_math_call(name: &str) -> Option<RuntimeHelper> {
    match name {
        "fabsf" => Some(RuntimeHelper::F32Fabs),
        "truncf" => Some(RuntimeHelper::F32Trunc),
        "floorf" => Some(RuntimeHelper::F32Floor),
        "ceilf" => Some(RuntimeHelper::F32Ceil),
        "roundf" => Some(RuntimeHelper::F32Round),
        "sqrtf" => Some(RuntimeHelper::F32Sqrt),
        _ => None,
    }
}

fn runtime_helper_for_float_compare(op: BinaryOp, ty: Type) -> Option<RuntimeHelper> {
    if ty.is_float()
        && matches!(
            op,
            BinaryOp::Equal
                | BinaryOp::NotEqual
                | BinaryOp::Less
                | BinaryOp::LessEqual
                | BinaryOp::Greater
                | BinaryOp::GreaterEqual
        )
    {
        Some(RuntimeHelper::F32Cmp)
    } else {
        None
    }
}

fn static_data_bytes(typed_program: &TypedProgram, layout: &StorageLayout) -> u16 {
    typed_program
        .symbols
        .iter()
        .filter(|symbol| symbol.fixed_address.is_none())
        .filter(|symbol| !symbol.ty.is_rom())
        .filter(|symbol| {
            symbol.kind == SymbolKind::Global
                || symbol.kind == SymbolKind::StringLiteral
                || (symbol.kind == SymbolKind::Local
                    && symbol.storage_class == StorageClass::Static)
        })
        .filter(|symbol| {
            matches!(
                layout.symbol_storage.get(&symbol.id),
                Some(SymbolStorage::Absolute(_))
            )
        })
        .fold(0u16, |sum, symbol| {
            sum.saturating_add(u16::try_from(symbol.ty.byte_width()).unwrap_or(u16::MAX))
        })
}

fn abi_slot_addresses(layout: &StorageLayout) -> BTreeSet<u16> {
    [
        layout.helpers.stack_ptr.lo,
        layout.helpers.stack_ptr.hi,
        layout.helpers.frame_ptr.lo,
        layout.helpers.frame_ptr.hi,
        layout.helpers.return_high,
        layout.helpers.return_upper0,
        layout.helpers.return_upper1,
        layout.helpers.scratch0,
        layout.helpers.scratch1,
        layout.helpers.flag_save,
        layout.helpers.w_save,
    ]
    .into_iter()
    .collect()
}

fn interrupt_context_addresses(layout: &StorageLayout) -> BTreeSet<u16> {
    let mut addresses = BTreeSet::new();
    if let Some(interrupt) = layout.interrupt {
        addresses.extend([
            interrupt.w,
            interrupt.status,
            interrupt.pclath,
            interrupt.fsr,
            interrupt.return_high,
            interrupt.return_upper0,
            interrupt.return_upper1,
            interrupt.scratch0,
            interrupt.scratch1,
            interrupt.flag_save,
            interrupt.w_save,
            interrupt.stack_ptr.lo,
            interrupt.stack_ptr.hi,
            interrupt.frame_ptr.lo,
            interrupt.frame_ptr.hi,
        ]);
    }
    addresses
}

/// Renders human-readable Phase 18 stack report text.
fn render_stack_report(
    target: &TargetDevice,
    typed_program: &TypedProgram,
    layout: &StorageLayout,
    analysis: &StackAnalysis,
    options: BackendOptions,
) -> String {
    use std::fmt::Write;

    let mut output = String::new();
    let _ = writeln!(output, "Stack report");
    let _ = writeln!(output, "------------");
    let _ = writeln!(output, "target: {}", target.name);
    let _ = writeln!(output, "growth: upward");
    let _ = writeln!(output, "base: 0x{:04X}", layout.stack_base);
    let _ = writeln!(output, "limit: 0x{:04X}", layout.stack_limit);
    let _ = writeln!(output, "capacity: {}", layout.stack_capacity);
    let _ = writeln!(output, "static max usage: {}", layout.max_stack_depth);
    let _ = writeln!(
        output,
        "runtime stack checks: {}",
        if options.stack_check {
            "enabled"
        } else {
            "disabled"
        }
    );
    let _ = writeln!(
        output,
        "isr frame bytes: {}",
        analysis.summary.isr_frame_bytes
    );
    let _ = writeln!(
        output,
        "isr context bytes: {}",
        analysis.summary.isr_context_bytes
    );
    let _ = writeln!(
        output,
        "function-pointer groups: {} targets={} unknown_target_sets={}",
        analysis.summary.function_pointer_groups,
        analysis.summary.function_pointer_targets,
        analysis.summary.unknown_function_pointer_target_sets
    );
    let _ = writeln!(output);
    let _ = writeln!(output, "Per-function");
    let _ = writeln!(output, "------------");
    for info in &analysis.functions {
        let symbol = &typed_program.symbols[info.symbol];
        let _ = writeln!(
            output,
            "{}: frame={} args={} locals={} temps={} helper_extra={} max_stack={} max_depth={}",
            symbol.name,
            info.frame.frame_bytes,
            info.frame.arg_bytes,
            info.frame.local_bytes,
            info.frame.temp_bytes,
            info.helper_extra,
            info.max_stack_bytes,
            info.max_call_depth
        );
        if !info.direct_callees.is_empty() {
            let names = info
                .direct_callees
                .iter()
                .map(|callee| typed_program.symbols[*callee].name.as_str())
                .collect::<Vec<_>>()
                .join(", ");
            let _ = writeln!(output, "  direct callees: {names}");
        }
        for group in &info.indirect_groups {
            if group.unknown {
                let _ = writeln!(
                    output,
                    "  indirect {} -> unknown target set",
                    group.signature
                );
            } else {
                let names = group
                    .targets
                    .iter()
                    .map(|callee| typed_program.symbols[*callee].name.as_str())
                    .collect::<Vec<_>>()
                    .join(", ");
                let _ = writeln!(output, "  indirect {} -> {}", group.signature, names);
            }
        }
    }
    let _ = writeln!(output);
    let _ = writeln!(
        output,
        "Recursion policy: rejected in phase 18. Runtime stack checks guard bounded acyclic growth only."
    );
    let _ = writeln!(
        output,
        "Trap: __stack_overflow_trap is infinite loop{}.",
        if options.stack_check {
            " when enabled"
        } else {
            " (not emitted unless --stack-check)"
        }
    );
    output
}

/// Builds the final map file from encoded labels and allocated data symbols.
fn build_map(
    typed_program: &TypedProgram,
    layout: &StorageLayout,
    labels: &BTreeMap<String, u16>,
    resource_lines: Vec<String>,
    sections: &[CodeSectionSummary],
    pages: &[PageLayoutSummary; 4],
) -> MapFile {
    let rom_label_names = typed_program
        .symbols
        .iter()
        .filter(|symbol| symbol.ty.is_rom())
        .map(|symbol| rom_object_label(symbol.id))
        .collect::<BTreeSet<_>>();
    let mut code_symbols = labels
        .iter()
        .filter(|(name, _)| !rom_label_names.contains(*name))
        .map(|(name, addr)| (name.clone(), *addr))
        .collect::<Vec<_>>();
    code_symbols.sort_by_key(|(_, addr)| *addr);

    let mut data_symbols = typed_program
        .symbols
        .iter()
        .filter(|symbol| symbol.kind != SymbolKind::Function)
        .filter_map(
            |symbol| match layout.symbol_storage.get(&symbol.id).copied() {
                Some(SymbolStorage::Absolute(addr)) => {
                    Some((format_data_symbol_name(symbol), addr))
                }
                Some(SymbolStorage::Frame(_)) | None => None,
            },
        )
        .collect::<Vec<_>>();
    data_symbols.extend([
        (
            "__abi.stack_ptr.lo".to_string(),
            layout.helpers.stack_ptr.lo,
        ),
        (
            "__abi.stack_ptr.hi".to_string(),
            layout.helpers.stack_ptr.hi,
        ),
        (
            "__abi.frame_ptr.lo".to_string(),
            layout.helpers.frame_ptr.lo,
        ),
        (
            "__abi.frame_ptr.hi".to_string(),
            layout.helpers.frame_ptr.hi,
        ),
        ("__stack_ptr".to_string(), layout.helpers.stack_ptr.lo),
        ("__stack_ptr.lo".to_string(), layout.helpers.stack_ptr.lo),
        ("__stack_ptr.hi".to_string(), layout.helpers.stack_ptr.hi),
        ("__frame_ptr".to_string(), layout.helpers.frame_ptr.lo),
        ("__frame_ptr.lo".to_string(), layout.helpers.frame_ptr.lo),
        ("__frame_ptr.hi".to_string(), layout.helpers.frame_ptr.hi),
        ("__abi.return_high".to_string(), layout.helpers.return_high),
        (
            "__abi.return_upper0".to_string(),
            layout.helpers.return_upper0,
        ),
        (
            "__abi.return_upper1".to_string(),
            layout.helpers.return_upper1,
        ),
        ("__abi.scratch0".to_string(), layout.helpers.scratch0),
        ("__abi.scratch1".to_string(), layout.helpers.scratch1),
        ("__abi.flag_save".to_string(), layout.helpers.flag_save),
        ("__abi.w_save".to_string(), layout.helpers.w_save),
        ("__stack.base".to_string(), layout.stack_base),
        ("__stack_base".to_string(), layout.stack_base),
        ("__stack.end".to_string(), layout.stack_end),
        ("__stack_limit".to_string(), layout.stack_limit),
    ]);
    if let Some(interrupt) = layout.interrupt {
        data_symbols.extend([
            ("__isr_ctx.w".to_string(), interrupt.w),
            ("__isr_ctx.status".to_string(), interrupt.status),
            ("__isr_ctx.pclath".to_string(), interrupt.pclath),
            ("__isr_ctx.fsr".to_string(), interrupt.fsr),
            ("__isr_ctx.return_high".to_string(), interrupt.return_high),
            (
                "__isr_ctx.return_upper0".to_string(),
                interrupt.return_upper0,
            ),
            (
                "__isr_ctx.return_upper1".to_string(),
                interrupt.return_upper1,
            ),
            ("__isr_ctx.scratch0".to_string(), interrupt.scratch0),
            ("__isr_ctx.scratch1".to_string(), interrupt.scratch1),
            ("__isr_ctx.flag_save".to_string(), interrupt.flag_save),
            ("__isr_ctx.w_save".to_string(), interrupt.w_save),
            ("__isr_ctx.stack_ptr.lo".to_string(), interrupt.stack_ptr.lo),
            ("__isr_ctx.stack_ptr.hi".to_string(), interrupt.stack_ptr.hi),
            ("__isr_ctx.frame_ptr.lo".to_string(), interrupt.frame_ptr.lo),
            ("__isr_ctx.frame_ptr.hi".to_string(), interrupt.frame_ptr.hi),
        ]);
    }
    data_symbols.sort_by_key(|(_, addr)| *addr);

    let mut rom_symbols = typed_program
        .symbols
        .iter()
        .filter(|symbol| symbol.ty.is_rom())
        .filter_map(|symbol| {
            labels
                .get(&rom_object_label(symbol.id))
                .copied()
                .map(|addr| (format_rom_symbol_name(symbol), addr))
        })
        .collect::<Vec<_>>();
    rom_symbols.sort_by_key(|(_, addr)| *addr);

    MapFile {
        resource_lines,
        code_layout_lines: render_code_layout_map_lines(sections, pages),
        code_symbols,
        data_symbols,
        rom_symbols,
    }
}

fn render_code_layout_map_lines(
    sections: &[CodeSectionSummary],
    pages: &[PageLayoutSummary; 4],
) -> Vec<String> {
    let mut lines = Vec::new();
    for page in pages {
        lines.push(format!(
            "page {}: used={} free={} range=0x{:04X}..0x{:04X}",
            page.page, page.used_words, page.free_words, page.start, page.end
        ));
        for section in sections
            .iter()
            .filter(|section| section.page_start <= page.page && section.page_end >= page.page)
        {
            lines.push(format!(
                "  0x{:04X}..0x{:04X} page {}..{} {:>4} words {:<27} {}",
                section.start,
                section.end,
                section.page_start,
                section.page_end,
                section.words,
                section.kind,
                section.name
            ));
        }
    }
    lines
}

/// Formats one data symbol with simple qualifiers that help map/listing readers.
fn format_data_symbol_name(symbol: &Symbol) -> String {
    let mut tags = Vec::new();
    if symbol.ty.object_is_const() {
        tags.push("const");
    }
    match (symbol.kind, symbol.storage_class) {
        (SymbolKind::StringLiteral, _) => tags.push("string literal"),
        (SymbolKind::Local, StorageClass::Static) => tags.push("static local"),
        (SymbolKind::Global, StorageClass::Static) => tags.push("static"),
        _ => {}
    }
    if tags.is_empty() {
        symbol.name.clone()
    } else {
        format!("{} [{}]", symbol.name, tags.join(", "))
    }
}

/// Formats one ROM symbol with tags that explain the callable RETLW-table representation.
fn format_rom_symbol_name(symbol: &Symbol) -> String {
    let mut tags = vec!["rom".to_string(), "const".to_string()];
    if symbol.storage_class == StorageClass::Static {
        tags.push("static".to_string());
    }
    if let Some(len) = symbol.ty.top_array_len() {
        let bytes = symbol.ty.byte_width();
        tags.push(format!("{len} element(s)"));
        tags.push(format!("{bytes} byte(s)"));
    }
    format!("{} [{}]", symbol.name, tags.join(", "))
}

/// Builds the assembly label used for one program-memory ROM table object.
fn rom_object_label(symbol: SymbolId) -> String {
    format!("__romobj{symbol}")
}

/// Evaluates a constant typed expression for startup initialization purposes.
fn eval_const_expr(expr: &TypedExpr) -> i64 {
    let target_ty = expr.ty;
    match &expr.kind {
        TypedExprKind::IntLiteral(value) => *value,
        TypedExprKind::Unary { op, expr } => {
            eval_unary(*op, eval_const_expr(expr), expr.ty, target_ty)
        }
        TypedExprKind::Binary { op, lhs, rhs } => eval_binary(
            *op,
            eval_const_expr(lhs),
            eval_const_expr(rhs),
            lhs.ty,
            target_ty,
        ),
        TypedExprKind::Cast {
            kind,
            expr: value_expr,
        } => {
            let value = eval_const_expr(value_expr);
            match kind {
                CastKind::ZeroExtend | CastKind::Truncate | CastKind::Bitcast => {
                    normalize_value(value, target_ty)
                }
                CastKind::SignExtend => {
                    normalize_value(signed_value(value, value_expr.ty), target_ty)
                }
                CastKind::F32ToQ16
                | CastKind::Q16ToF32
                | CastKind::I32ToF32
                | CastKind::U32ToF32
                | CastKind::F32ToI32
                | CastKind::F32ToU32 => eval_float_cast_constant(value, value_expr.ty, target_ty),
            }
        }
        TypedExprKind::Assign { .. }
        | TypedExprKind::BitField { .. }
        | TypedExprKind::StructAssign { .. }
        | TypedExprKind::RomRead8 { .. }
        | TypedExprKind::RomRead16 { .. }
        | TypedExprKind::RomRead32 { .. }
        | TypedExprKind::Call { .. }
        | TypedExprKind::IndirectCall { .. }
        | TypedExprKind::ArrayDecay(_)
        | TypedExprKind::AddressOf(_)
        | TypedExprKind::Deref(_)
        | TypedExprKind::Symbol(_) => 0,
    }
}

fn eval_float_cast_constant(value: i64, source_ty: Type, target_ty: Type) -> i64 {
    let float_value = if source_ty.is_float() {
        f32::from_bits(normalize_value(value, source_ty) as u32)
    } else if source_ty.is_fixed() {
        let raw = if source_ty.is_signed() {
            signed_value(value, source_ty) as f32
        } else {
            normalize_value(value, source_ty) as f32
        };
        raw / ((1_u32 << source_ty.fixed_fraction_bits().unwrap_or(0)) as f32)
    } else if source_ty.is_signed() {
        signed_value(value, source_ty) as f32
    } else {
        normalize_value(value, source_ty) as f32
    };
    if target_ty.is_float() {
        return i64::from(float_value.to_bits());
    }
    if target_ty.is_fixed() {
        return normalize_value(
            (float_value * ((1_u32 << target_ty.fixed_fraction_bits().unwrap_or(0)) as f32)).trunc()
                as i64,
            target_ty,
        );
    }
    normalize_value(float_value.trunc() as i64, target_ty)
}

/// Returns one constant operand normalized to the destination type when available.
fn constant_operand_value(operand: Operand, ty: Type) -> Option<i64> {
    match operand {
        Operand::Constant(value) => Some(normalize_value(value, ty)),
        Operand::Symbol(_) | Operand::Temp(_) => None,
    }
}

/// Returns the shift amount when a normalized constant is an exact power of two.
fn normalized_power_of_two_shift(value: Option<i64>, ty: Type) -> Option<usize> {
    let value = normalize_value(value?, ty) as u64;
    if value == 0 || !value.is_power_of_two() {
        return None;
    }
    Some(value.trailing_zeros() as usize)
}

/// Returns the `(divisor - 1)` mask when a normalized constant is an exact power of two.
fn normalized_power_of_two_mask(value: Option<i64>, ty: Type) -> Option<i64> {
    let value = normalize_value(value?, ty);
    if value == 0 {
        return None;
    }
    let unsigned = value as u64;
    if !unsigned.is_power_of_two() {
        return None;
    }
    Some(normalize_value(value - 1, ty))
}

/// Returns the low seven address bits used by direct-register PIC16 instructions.
const fn low7(addr: u16) -> u8 {
    (addr & 0x7F) as u8
}

/// Returns the two's-complement negation of a 16-bit byte count.
const fn negate_u16(value: u16) -> u16 {
    value.wrapping_neg()
}

#[cfg(test)]
mod tests {
    use super::{
        BackendOptions, CodegenContext, FrameLayout, StorageAllocator, compile_program,
        compute_max_stack_depth, low7,
    };
    use crate::backend::pic16::devices::DeviceRegistry;
    use crate::backend::pic16::midrange14::asm::{AsmInstr, AsmLine};
    use crate::common::source::Span;
    use crate::diagnostics::{DiagnosticBag, WarningProfile};
    use crate::frontend::ast::BinaryOp;
    use crate::frontend::semantic::{Symbol, SymbolKind, TypedFunction, TypedGlobal, TypedProgram};
    use crate::frontend::types::{ScalarType, StorageClass, Type};
    use crate::ir::model::{IrBlock, IrFunction, IrInstr, IrProgram, IrTerminator, Operand};
    use std::collections::BTreeMap;

    #[test]
    /// Verifies indirect loads and stores emit the expected `FSR/INDF` assembly pattern.
    fn phase_three_indirect_memory_ops_use_fsr_and_indf() {
        let registry = DeviceRegistry::new();
        let target = registry.device("pic16f628a").expect("device");
        let pointer_ty = Type::new(ScalarType::U8).pointer_to();
        let byte_ty = Type::new(ScalarType::U8);
        let program = TypedProgram {
            symbols: vec![
                symbol(0, "main", Type::new(ScalarType::Void), SymbolKind::Function),
                symbol(1, "bytes", byte_ty.array_of(2), SymbolKind::Global),
            ],
            globals: vec![TypedGlobal {
                symbol: 1,
                initializer: None,
            }],
            functions: vec![TypedFunction {
                symbol: 0,
                params: Vec::new(),
                locals: Vec::new(),
                body: None,
                return_type: Type::new(ScalarType::Void),
                span: Span::new(0, 0),
            }],
            function_pointer_groups: Vec::new(),
        };
        let ir = IrProgram {
            globals: vec![1],
            functions: vec![IrFunction {
                symbol: 0,
                is_interrupt: false,
                params: Vec::new(),
                locals: Vec::new(),
                entry: 0,
                temp_types: vec![pointer_ty, byte_ty],
                return_type: Type::new(ScalarType::Void),
                blocks: vec![IrBlock {
                    id: 0,
                    name: "entry".to_string(),
                    instructions: vec![
                        IrInstr::AddrOf { dst: 0, symbol: 1 },
                        IrInstr::StoreIndirect {
                            ptr: Operand::Temp(0),
                            value: Operand::Constant(0x34),
                            ty: byte_ty,
                        },
                        IrInstr::LoadIndirect {
                            dst: 1,
                            ptr: Operand::Temp(0),
                        },
                    ],
                    terminator: IrTerminator::Return(None),
                }],
            }],
        };
        let mut diagnostics = DiagnosticBag::new(WarningProfile::default());
        let output = compile_program(
            target,
            &program,
            &ir,
            &BackendOptions::default(),
            &mut diagnostics,
        )
        .expect("backend");

        assert!(!diagnostics.has_errors());
        let asm = output.program.render();
        assert!(asm.contains("movwf 0x04"));
        assert!(asm.contains("movwf 0x00"));
        assert!(asm.contains("movf 0x00,w"));
    }

    #[test]
    /// Verifies stack-depth analysis includes both frame bytes and caller-pushed argument bytes.
    fn phase_four_stack_depth_accounts_for_frames_and_args() {
        let program = IrProgram {
            globals: Vec::new(),
            functions: vec![
                IrFunction {
                    symbol: 1,
                    is_interrupt: false,
                    params: Vec::new(),
                    locals: Vec::new(),
                    entry: 0,
                    temp_types: Vec::new(),
                    return_type: Type::new(ScalarType::Void),
                    blocks: vec![IrBlock {
                        id: 0,
                        name: "entry".to_string(),
                        instructions: vec![IrInstr::Call {
                            dst: None,
                            function: 2,
                            args: vec![
                                Operand::Constant(1),
                                Operand::Constant(2),
                                Operand::Constant(3),
                            ],
                        }],
                        terminator: IrTerminator::Return(None),
                    }],
                },
                IrFunction {
                    symbol: 2,
                    is_interrupt: false,
                    params: Vec::new(),
                    locals: Vec::new(),
                    entry: 0,
                    temp_types: Vec::new(),
                    return_type: Type::new(ScalarType::Void),
                    blocks: vec![IrBlock {
                        id: 0,
                        name: "entry".to_string(),
                        instructions: Vec::new(),
                        terminator: IrTerminator::Return(None),
                    }],
                },
            ],
        };
        let frames = BTreeMap::from([
            (
                1,
                FrameLayout {
                    arg_bytes: 0,
                    saved_fp_offset: 0,
                    local_bytes: 2,
                    temp_bytes: 4,
                    frame_bytes: 6,
                },
            ),
            (
                2,
                FrameLayout {
                    arg_bytes: 6,
                    saved_fp_offset: 6,
                    local_bytes: 0,
                    temp_bytes: 2,
                    frame_bytes: 4,
                },
            ),
        ]);

        assert_eq!(compute_max_stack_depth(&program, &frames), 16);
    }

    #[test]
    /// Verifies Phase 4 function prologues emit software-stack setup comments and calls survive.
    fn phase_four_stack_abi_emits_stack_metadata() {
        let registry = DeviceRegistry::new();
        let target = registry.device("pic16f628a").expect("device");
        let u16_ty = Type::new(ScalarType::U16);
        let mut sum3_symbol = symbol(1, "sum3", u16_ty, SymbolKind::Function);
        sum3_symbol.parameter_types = vec![u16_ty, u16_ty, u16_ty];
        let program = TypedProgram {
            symbols: vec![
                symbol(0, "main", Type::new(ScalarType::Void), SymbolKind::Function),
                sum3_symbol,
                symbol(2, "a", u16_ty, SymbolKind::Param),
                symbol(3, "b", u16_ty, SymbolKind::Param),
                symbol(4, "c", u16_ty, SymbolKind::Param),
            ],
            globals: Vec::new(),
            functions: vec![
                TypedFunction {
                    symbol: 0,
                    params: Vec::new(),
                    locals: Vec::new(),
                    body: None,
                    return_type: Type::new(ScalarType::Void),
                    span: Span::new(0, 0),
                },
                TypedFunction {
                    symbol: 1,
                    params: vec![2, 3, 4],
                    locals: Vec::new(),
                    body: None,
                    return_type: u16_ty,
                    span: Span::new(0, 0),
                },
            ],
            function_pointer_groups: Vec::new(),
        };
        let ir = IrProgram {
            globals: Vec::new(),
            functions: vec![
                IrFunction {
                    symbol: 0,
                    is_interrupt: false,
                    params: Vec::new(),
                    locals: Vec::new(),
                    entry: 0,
                    temp_types: vec![u16_ty],
                    return_type: Type::new(ScalarType::Void),
                    blocks: vec![IrBlock {
                        id: 0,
                        name: "entry".to_string(),
                        instructions: vec![IrInstr::Call {
                            dst: Some(0),
                            function: 1,
                            args: vec![
                                Operand::Constant(1),
                                Operand::Constant(2),
                                Operand::Constant(3),
                            ],
                        }],
                        terminator: IrTerminator::Return(None),
                    }],
                },
                IrFunction {
                    symbol: 1,
                    is_interrupt: false,
                    params: vec![2, 3, 4],
                    locals: Vec::new(),
                    entry: 0,
                    temp_types: Vec::new(),
                    return_type: u16_ty,
                    blocks: vec![IrBlock {
                        id: 0,
                        name: "entry".to_string(),
                        instructions: Vec::new(),
                        terminator: IrTerminator::Return(Some(Operand::Constant(0))),
                    }],
                },
            ],
        };
        let mut diagnostics = DiagnosticBag::new(WarningProfile::default());
        let output = compile_program(
            target,
            &program,
            &ir,
            &BackendOptions::default(),
            &mut diagnostics,
        )
        .expect("backend");

        assert!(!diagnostics.has_errors());
        let asm = output.program.render();
        assert!(asm.contains("stack base="));
        assert!(asm.contains("call fn_sum3"));
        assert!(
            output
                .map
                .data_symbols
                .iter()
                .any(|(name, _)| name == "__abi.stack_ptr.lo")
        );
        assert!(
            output
                .map
                .data_symbols
                .iter()
                .any(|(name, _)| name == "__stack.base")
        );
    }

    #[test]
    /// Verifies IR temps live inside the dynamic frame instead of static absolute RAM.
    fn phase_four_temps_live_in_frame_storage() {
        let registry = DeviceRegistry::new();
        let target = registry.device("pic16f628a").expect("device");
        let u16_ty = Type::new(ScalarType::U16);
        let program = TypedProgram {
            symbols: vec![
                symbol(0, "main", Type::new(ScalarType::Void), SymbolKind::Function),
                symbol(1, "local", u16_ty, SymbolKind::Local),
            ],
            globals: Vec::new(),
            functions: vec![TypedFunction {
                symbol: 0,
                params: Vec::new(),
                locals: vec![1],
                body: None,
                return_type: Type::new(ScalarType::Void),
                span: Span::new(0, 0),
            }],
            function_pointer_groups: Vec::new(),
        };
        let ir = IrProgram {
            globals: Vec::new(),
            functions: vec![IrFunction {
                symbol: 0,
                is_interrupt: false,
                params: Vec::new(),
                locals: vec![1],
                entry: 0,
                temp_types: vec![u16_ty, u16_ty],
                return_type: Type::new(ScalarType::Void),
                blocks: vec![IrBlock {
                    id: 0,
                    name: "entry".to_string(),
                    instructions: vec![
                        IrInstr::Copy {
                            dst: 0,
                            src: Operand::Constant(1),
                        },
                        IrInstr::Binary {
                            dst: 1,
                            op: BinaryOp::Add,
                            lhs: Operand::Temp(0),
                            rhs: Operand::Constant(2),
                        },
                        IrInstr::Store {
                            target: 1,
                            value: Operand::Temp(1),
                        },
                    ],
                    terminator: IrTerminator::Return(None),
                }],
            }],
        };
        let mut diagnostics = DiagnosticBag::new(WarningProfile::default());
        let layout = StorageAllocator::new(target.allocatable_gpr, target.shared_gpr)
            .layout(&program, &ir, &mut diagnostics)
            .expect("layout");

        assert!(!diagnostics.has_errors());
        let frame = &layout.frames[&0];
        assert_eq!(frame.local_bytes, 2);
        assert_eq!(frame.temp_bytes, 4);
        assert_eq!(layout.temp_offsets[&(0, 0)], 4);
        assert_eq!(layout.temp_offsets[&(0, 1)], 6);
    }

    #[test]
    /// Verifies function epilogues restore `SP` from the active frame before restoring caller `FP`.
    fn phase_four_epilogue_restores_sp_before_fp() {
        let registry = DeviceRegistry::new();
        let target = registry.device("pic16f628a").expect("device");
        let u16_ty = Type::new(ScalarType::U16);
        let mut add2_symbol = symbol(1, "add2", u16_ty, SymbolKind::Function);
        add2_symbol.parameter_types = vec![u16_ty, u16_ty];
        let program = TypedProgram {
            symbols: vec![
                symbol(0, "main", Type::new(ScalarType::Void), SymbolKind::Function),
                add2_symbol,
                symbol(2, "a", u16_ty, SymbolKind::Param),
                symbol(3, "b", u16_ty, SymbolKind::Param),
            ],
            globals: Vec::new(),
            functions: vec![
                TypedFunction {
                    symbol: 0,
                    params: Vec::new(),
                    locals: Vec::new(),
                    body: None,
                    return_type: Type::new(ScalarType::Void),
                    span: Span::new(0, 0),
                },
                TypedFunction {
                    symbol: 1,
                    params: vec![2, 3],
                    locals: Vec::new(),
                    body: None,
                    return_type: u16_ty,
                    span: Span::new(0, 0),
                },
            ],
            function_pointer_groups: Vec::new(),
        };
        let ir = IrProgram {
            globals: Vec::new(),
            functions: vec![
                IrFunction {
                    symbol: 0,
                    is_interrupt: false,
                    params: Vec::new(),
                    locals: Vec::new(),
                    entry: 0,
                    temp_types: vec![u16_ty, u16_ty],
                    return_type: Type::new(ScalarType::Void),
                    blocks: vec![IrBlock {
                        id: 0,
                        name: "entry".to_string(),
                        instructions: vec![
                            IrInstr::Call {
                                dst: Some(0),
                                function: 1,
                                args: vec![Operand::Constant(1), Operand::Constant(2)],
                            },
                            IrInstr::Call {
                                dst: Some(1),
                                function: 1,
                                args: vec![Operand::Constant(3), Operand::Constant(4)],
                            },
                        ],
                        terminator: IrTerminator::Return(None),
                    }],
                },
                IrFunction {
                    symbol: 1,
                    is_interrupt: false,
                    params: vec![2, 3],
                    locals: Vec::new(),
                    entry: 0,
                    temp_types: vec![u16_ty],
                    return_type: u16_ty,
                    blocks: vec![IrBlock {
                        id: 0,
                        name: "entry".to_string(),
                        instructions: vec![IrInstr::Binary {
                            dst: 0,
                            op: BinaryOp::Add,
                            lhs: Operand::Symbol(2),
                            rhs: Operand::Symbol(3),
                        }],
                        terminator: IrTerminator::Return(Some(Operand::Temp(0))),
                    }],
                },
            ],
        };
        let mut diagnostics = DiagnosticBag::new(WarningProfile::default());
        let layout = StorageAllocator::new(target.allocatable_gpr, target.shared_gpr)
            .layout(&program, &ir, &mut diagnostics)
            .expect("layout");
        let mut codegen = CodegenContext::new(target, &program, &layout, BackendOptions::default());
        codegen.emit_program(&ir, &mut diagnostics);

        assert!(!diagnostics.has_errors());
        let lines = &codegen.program.lines;
        let return_index = lines
            .iter()
            .rposition(|line| matches!(line, AsmLine::Instr(AsmInstr::Return)))
            .expect("return");
        let last_sp_restore = lines[..return_index]
            .iter()
            .rposition(|line| {
                matches!(line, AsmLine::Instr(AsmInstr::Movwf(f)) if *f == low7(layout.helpers.stack_ptr.lo))
            })
            .expect("sp restore");
        let last_fp_restore = lines[..return_index]
            .iter()
            .rposition(|line| {
                matches!(line, AsmLine::Instr(AsmInstr::Movwf(f)) if *f == low7(layout.helpers.frame_ptr.lo))
            })
            .expect("fp restore");

        assert!(last_sp_restore < last_fp_restore);
    }

    #[test]
    /// Verifies stack-depth analysis includes compiler-generated Phase 5 helper calls.
    fn phase_five_stack_depth_accounts_for_runtime_helpers() {
        let u16_ty = Type::new(ScalarType::U16);
        let program = IrProgram {
            globals: Vec::new(),
            functions: vec![IrFunction {
                symbol: 1,
                is_interrupt: false,
                params: Vec::new(),
                locals: Vec::new(),
                entry: 0,
                temp_types: vec![u16_ty],
                return_type: Type::new(ScalarType::Void),
                blocks: vec![IrBlock {
                    id: 0,
                    name: "entry".to_string(),
                    instructions: vec![IrInstr::Binary {
                        dst: 0,
                        op: BinaryOp::Multiply,
                        lhs: Operand::Constant(3),
                        rhs: Operand::Constant(7),
                    }],
                    terminator: IrTerminator::Return(None),
                }],
            }],
        };
        let frames = BTreeMap::from([(
            1,
            FrameLayout {
                arg_bytes: 0,
                saved_fp_offset: 0,
                local_bytes: 0,
                temp_bytes: 2,
                frame_bytes: 4,
            },
        )]);

        assert_eq!(compute_max_stack_depth(&program, &frames), 14);
    }

    #[test]
    /// Verifies Phase 5 multiplication lowers through helper calls that appear in code symbols.
    fn phase_five_runtime_helper_calls_emit_labels_and_calls() {
        let registry = DeviceRegistry::new();
        let target = registry.device("pic16f628a").expect("device");
        let u16_ty = Type::new(ScalarType::U16);
        let program = TypedProgram {
            symbols: vec![symbol(
                0,
                "main",
                Type::new(ScalarType::Void),
                SymbolKind::Function,
            )],
            globals: Vec::new(),
            functions: vec![TypedFunction {
                symbol: 0,
                params: Vec::new(),
                locals: Vec::new(),
                body: None,
                return_type: Type::new(ScalarType::Void),
                span: Span::new(0, 0),
            }],
            function_pointer_groups: Vec::new(),
        };
        let ir = IrProgram {
            globals: Vec::new(),
            functions: vec![IrFunction {
                symbol: 0,
                is_interrupt: false,
                params: Vec::new(),
                locals: Vec::new(),
                entry: 0,
                temp_types: vec![u16_ty],
                return_type: Type::new(ScalarType::Void),
                blocks: vec![IrBlock {
                    id: 0,
                    name: "entry".to_string(),
                    instructions: vec![IrInstr::Binary {
                        dst: 0,
                        op: BinaryOp::Multiply,
                        lhs: Operand::Constant(9),
                        rhs: Operand::Constant(11),
                    }],
                    terminator: IrTerminator::Return(None),
                }],
            }],
        };
        let mut diagnostics = DiagnosticBag::new(WarningProfile::default());
        let output = compile_program(
            target,
            &program,
            &ir,
            &BackendOptions::default(),
            &mut diagnostics,
        )
        .expect("backend");

        assert!(!diagnostics.has_errors());
        let asm = output.program.render();
        assert!(asm.contains("call __rt_mul_u16"));
        assert!(asm.contains("__rt_mul_u16:"));
        assert!(
            output
                .map
                .code_symbols
                .iter()
                .any(|(name, _)| name == "__rt_mul_u16")
        );
    }

    #[test]
    /// Verifies constant-count shifts lower inline without pulling in the dynamic helper path.
    fn phase_five_constant_shift_stays_inline() {
        let registry = DeviceRegistry::new();
        let target = registry.device("pic16f628a").expect("device");
        let u16_ty = Type::new(ScalarType::U16);
        let program = TypedProgram {
            symbols: vec![symbol(
                0,
                "main",
                Type::new(ScalarType::Void),
                SymbolKind::Function,
            )],
            globals: Vec::new(),
            functions: vec![TypedFunction {
                symbol: 0,
                params: Vec::new(),
                locals: Vec::new(),
                body: None,
                return_type: Type::new(ScalarType::Void),
                span: Span::new(0, 0),
            }],
            function_pointer_groups: Vec::new(),
        };
        let ir = IrProgram {
            globals: Vec::new(),
            functions: vec![IrFunction {
                symbol: 0,
                is_interrupt: false,
                params: Vec::new(),
                locals: Vec::new(),
                entry: 0,
                temp_types: vec![u16_ty],
                return_type: Type::new(ScalarType::Void),
                blocks: vec![IrBlock {
                    id: 0,
                    name: "entry".to_string(),
                    instructions: vec![IrInstr::Binary {
                        dst: 0,
                        op: BinaryOp::ShiftRight,
                        lhs: Operand::Constant(0x0123),
                        rhs: Operand::Constant(3),
                    }],
                    terminator: IrTerminator::Return(None),
                }],
            }],
        };
        let mut diagnostics = DiagnosticBag::new(WarningProfile::default());
        let output = compile_program(
            target,
            &program,
            &ir,
            &BackendOptions::default(),
            &mut diagnostics,
        )
        .expect("backend");

        assert!(!diagnostics.has_errors());
        let asm = output.program.render();
        assert!(!asm.contains("call __rt_shr_u16"));
        assert!(!asm.contains("__rt_shr_u16:"));
    }

    #[test]
    /// Verifies a program without an ISR leaves the interrupt vector as a safe `retfie`.
    fn phase_six_default_interrupt_vector_is_retfie() {
        let registry = DeviceRegistry::new();
        let target = registry.device("pic16f628a").expect("device");
        let program = TypedProgram {
            symbols: vec![symbol(
                0,
                "main",
                Type::new(ScalarType::Void),
                SymbolKind::Function,
            )],
            globals: Vec::new(),
            functions: vec![TypedFunction {
                symbol: 0,
                params: Vec::new(),
                locals: Vec::new(),
                body: None,
                return_type: Type::new(ScalarType::Void),
                span: Span::new(0, 0),
            }],
            function_pointer_groups: Vec::new(),
        };
        let ir = IrProgram {
            globals: Vec::new(),
            functions: vec![IrFunction {
                symbol: 0,
                is_interrupt: false,
                params: Vec::new(),
                locals: Vec::new(),
                entry: 0,
                temp_types: Vec::new(),
                return_type: Type::new(ScalarType::Void),
                blocks: vec![IrBlock {
                    id: 0,
                    name: "entry".to_string(),
                    instructions: Vec::new(),
                    terminator: IrTerminator::Return(None),
                }],
            }],
        };
        let mut diagnostics = DiagnosticBag::new(WarningProfile::default());
        let output = compile_program(
            target,
            &program,
            &ir,
            &BackendOptions::default(),
            &mut diagnostics,
        )
        .expect("backend");

        assert!(!diagnostics.has_errors());
        assert_eq!(output.words[&target.vectors.interrupt], 0x0009);
    }

    #[test]
    /// Verifies Phase 6 emits vector dispatch, ISR context slots, and `retfie` for one handler.
    fn phase_six_interrupt_vectors_and_context_emit() {
        let registry = DeviceRegistry::new();
        let target = registry.device("pic16f628a").expect("device");
        let mut isr = symbol(1, "isr", Type::new(ScalarType::Void), SymbolKind::Function);
        isr.is_interrupt = true;
        let program = TypedProgram {
            symbols: vec![
                symbol(0, "main", Type::new(ScalarType::Void), SymbolKind::Function),
                isr,
            ],
            globals: Vec::new(),
            functions: vec![
                TypedFunction {
                    symbol: 0,
                    params: Vec::new(),
                    locals: Vec::new(),
                    body: None,
                    return_type: Type::new(ScalarType::Void),
                    span: Span::new(0, 0),
                },
                TypedFunction {
                    symbol: 1,
                    params: Vec::new(),
                    locals: Vec::new(),
                    body: None,
                    return_type: Type::new(ScalarType::Void),
                    span: Span::new(0, 0),
                },
            ],
            function_pointer_groups: Vec::new(),
        };
        let ir = IrProgram {
            globals: Vec::new(),
            functions: vec![
                IrFunction {
                    symbol: 0,
                    is_interrupt: false,
                    params: Vec::new(),
                    locals: Vec::new(),
                    entry: 0,
                    temp_types: Vec::new(),
                    return_type: Type::new(ScalarType::Void),
                    blocks: vec![IrBlock {
                        id: 0,
                        name: "entry".to_string(),
                        instructions: Vec::new(),
                        terminator: IrTerminator::Return(None),
                    }],
                },
                IrFunction {
                    symbol: 1,
                    is_interrupt: true,
                    params: Vec::new(),
                    locals: Vec::new(),
                    entry: 0,
                    temp_types: Vec::new(),
                    return_type: Type::new(ScalarType::Void),
                    blocks: vec![IrBlock {
                        id: 0,
                        name: "entry".to_string(),
                        instructions: Vec::new(),
                        terminator: IrTerminator::Return(None),
                    }],
                },
            ],
        };
        let mut diagnostics = DiagnosticBag::new(WarningProfile::default());
        let output = compile_program(
            target,
            &program,
            &ir,
            &BackendOptions::default(),
            &mut diagnostics,
        )
        .expect("backend");

        assert!(!diagnostics.has_errors());
        let reset_dispatch = output
            .map
            .code_symbols
            .iter()
            .find(|(name, _)| name == "__reset_dispatch")
            .expect("reset dispatch")
            .1;
        let interrupt_dispatch = output
            .map
            .code_symbols
            .iter()
            .find(|(name, _)| name == "__interrupt_dispatch")
            .expect("interrupt dispatch")
            .1;
        assert_eq!(
            output.words[&target.vectors.reset],
            0x2800 | (reset_dispatch & 0x07FF)
        );
        assert_eq!(
            output.words[&target.vectors.interrupt],
            0x2800 | (interrupt_dispatch & 0x07FF)
        );
        assert!(
            output
                .map
                .data_symbols
                .iter()
                .any(|(name, _)| name == "__isr_ctx.w")
        );
        assert!(
            output
                .map
                .data_symbols
                .iter()
                .any(|(name, _)| name == "__isr_ctx.stack_ptr.lo")
        );
        assert!(output.program.render().contains("retfie"));
    }

    /// Builds one typed symbol used by the backend unit test fixture.
    fn symbol(id: usize, name: &str, ty: Type, kind: SymbolKind) -> Symbol {
        Symbol {
            id,
            name: name.to_string(),
            ty,
            storage_class: StorageClass::Auto,
            is_interrupt: false,
            kind,
            span: Span::new(0, 0),
            fixed_address: None,
            is_defined: true,
            is_referenced: true,
            parameter_types: Vec::new(),
            enum_const_value: None,
        }
    }
}
// SPDX-License-Identifier: GPL-3.0-or-later
