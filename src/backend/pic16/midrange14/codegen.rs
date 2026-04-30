use std::collections::{BTreeMap, BTreeSet};

use crate::backend::pic16::devices::{MemoryRange, TargetDevice};
use crate::common::integer::{
    compare_rel, eval_binary, eval_unary, high_byte, low_byte, normalize_value, signed_value,
};
use crate::diagnostics::DiagnosticBag;
use crate::frontend::ast::{BinaryOp, UnaryOp};
use crate::frontend::semantic::{
    Symbol, SymbolId, SymbolKind, TypedExpr, TypedExprKind, TypedGlobalInitializer, TypedProgram,
};
use crate::frontend::types::{CastKind, ScalarType, StorageClass, Type};
use crate::ir::model::{IrCondition, IrFunction, IrInstr, IrProgram, IrTerminator, Operand};
use crate::linker::map::MapFile;

use super::asm::{AsmInstr, AsmLine, AsmProgram, Dest, PeepholeStats};
use super::encoder::encode_program;
use super::runtime::{binary_helper, RuntimeHelper, RuntimeHelperInfo};

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
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct BackendOptimizationReport {
    pub peephole: PeepholeStats,
    pub helper_calls_avoided: usize,
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
    scratch0: u16,
    scratch1: u16,
}

#[derive(Clone, Copy, Debug)]
struct InterruptContext {
    w: u16,
    status: u16,
    pclath: u16,
    fsr: u16,
    return_high: u16,
    scratch0: u16,
    scratch1: u16,
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

/// Lowers typed IR into assembly, encoded words, and a final linker map.
pub fn compile_program(
    target: &TargetDevice,
    typed_program: &TypedProgram,
    ir_program: &IrProgram,
    diagnostics: &mut DiagnosticBag,
) -> Option<BackendOutput> {
    let layout = StorageAllocator::new(target.allocatable_gpr, target.shared_gpr)
        .layout(typed_program, ir_program, diagnostics)?;

    let mut codegen = CodegenContext::new(target, typed_program, &layout);
    codegen.emit_program(ir_program, diagnostics);
    if diagnostics.has_errors() {
        return None;
    }

    let optimization = codegen.optimize_program();

    let encoded = encode_program(&codegen.program, diagnostics)?;
    let map = build_map(typed_program, &layout, &encoded.labels);
    Some(BackendOutput {
        program: codegen.program,
        words: encoded.words,
        map,
        optimization,
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
        let Some(scratch0) = allocator.next_span(1) else {
            diagnostics.error("backend", None, "not enough RAM for ABI helper slots", None);
            return None;
        };
        let Some(scratch1) = allocator.next_span(1) else {
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
            scratch0,
            scratch1,
        };

        let interrupt = if ir_program.functions.iter().any(|function| function.is_interrupt) {
            let mut shared = AddressAllocator::new(self.shared_ranges);
            let Some(w) = shared.next_span(1) else {
                diagnostics.error("backend", None, "not enough shared RAM for ISR context", None);
                return None;
            };
            let Some(status) = shared.next_span(1) else {
                diagnostics.error("backend", None, "not enough shared RAM for ISR context", None);
                return None;
            };
            let Some(pclath) = shared.next_span(1) else {
                diagnostics.error("backend", None, "not enough shared RAM for ISR context", None);
                return None;
            };
            let Some(fsr) = shared.next_span(1) else {
                diagnostics.error("backend", None, "not enough shared RAM for ISR context", None);
                return None;
            };
            let Some(return_high_ctx) = shared.next_span(1) else {
                diagnostics.error("backend", None, "not enough shared RAM for ISR context", None);
                return None;
            };
            let Some(scratch0_ctx) = shared.next_span(1) else {
                diagnostics.error("backend", None, "not enough shared RAM for ISR context", None);
                return None;
            };
            let Some(scratch1_ctx) = shared.next_span(1) else {
                diagnostics.error("backend", None, "not enough shared RAM for ISR context", None);
                return None;
            };
            let Some(stack_ptr_ctx_lo) = shared.next_span(2) else {
                diagnostics.error("backend", None, "not enough shared RAM for ISR context", None);
                return None;
            };
            let Some(frame_ptr_ctx_lo) = shared.next_span(2) else {
                diagnostics.error("backend", None, "not enough shared RAM for ISR context", None);
                return None;
            };

            Some(InterruptContext {
                w,
                status,
                pclath,
                fsr,
                return_high: return_high_ctx,
                scratch0: scratch0_ctx,
                scratch1: scratch1_ctx,
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
                || (symbol.kind == SymbolKind::Local && symbol.storage_class == crate::frontend::types::StorageClass::Static)
            {
                let Some(base) = allocator.next_span(symbol.ty.byte_width()) else {
                    diagnostics.error(
                        "backend",
                        None,
                        format!("not enough allocatable RAM for symbol `{}`", symbol.name),
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
                if symbol.kind != SymbolKind::Local || symbol.storage_class == crate::frontend::types::StorageClass::Static {
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
                "not enough RAM left for the Phase 4 software stack",
                None,
            );
            return None;
        };

        let max_stack_depth = compute_max_stack_depth_with_interrupts(ir_program, &frames);
        if max_stack_depth > stack_capacity {
            diagnostics.error(
                "backend",
                None,
                format!(
                    "Phase 4 software stack needs {max_stack_depth} bytes but only {stack_capacity} bytes remain"
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
    program: AsmProgram,
    current_bank: u8,
    label_counter: usize,
    used_helpers: BTreeSet<RuntimeHelper>,
    helper_calls_avoided: usize,
}

impl<'a> CodegenContext<'a> {
    /// Creates a backend codegen context for one target, program, and storage layout.
    fn new(target: &'a TargetDevice, typed_program: &'a TypedProgram, layout: &'a StorageLayout) -> Self {
        Self {
            target,
            typed_program,
            layout,
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
        self.emit_runtime_helpers();
    }

    /// Applies backend-local optimization passes and returns a summary for reporting.
    fn optimize_program(&mut self) -> BackendOptimizationReport {
        BackendOptimizationReport {
            peephole: self.program.peephole_optimize(),
            helper_calls_avoided: self.helper_calls_avoided,
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
        self.program.push(AsmLine::Label("__reset_vector".to_string()));
        self.program
            .push(AsmLine::Instr(AsmInstr::Goto("__reset_dispatch".to_string())));
        self.program.push(AsmLine::Org(self.target.vectors.interrupt));
        self.program.push(AsmLine::Label("__interrupt_vector".to_string()));
        if interrupt.is_some() {
            self.program
                .push(AsmLine::Instr(AsmInstr::Goto("__interrupt_dispatch".to_string())));
        } else {
            self.program.push(AsmLine::Instr(AsmInstr::Retfie));
        }
        self.program.push(AsmLine::Org(self.target.vectors.interrupt + 1));
        self.program.push(AsmLine::Label("__reset_dispatch".to_string()));
        self.program.push(AsmLine::Instr(AsmInstr::SetPage("__start".to_string())));
        self.program.push(AsmLine::Instr(AsmInstr::Goto("__start".to_string())));
        if let Some(interrupt) = interrupt {
            self.program
                .push(AsmLine::Label("__interrupt_dispatch".to_string()));
            self.program.push(AsmLine::Instr(AsmInstr::SetPage(interrupt.clone())));
            self.program.push(AsmLine::Instr(AsmInstr::Goto(interrupt)));
        }
        self.program.push(AsmLine::Label("__start".to_string()));
    }

    /// Emits startup initialization for globals and transfers control to `main`.
    fn emit_startup(&mut self, ir_program: &IrProgram, diagnostics: &mut DiagnosticBag) {
        self.program.push(AsmLine::Comment(format!(
            "stack base=0x{:04X} end=0x{:04X} capacity={} max_depth={}",
            self.layout.stack_base,
            self.layout.stack_end,
            self.layout.stack_capacity,
            self.layout.max_stack_depth
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
        self.program.push(AsmLine::Instr(AsmInstr::SetPage(label.clone())));
        self.program.push(AsmLine::Instr(AsmInstr::Call(label)));
        self.program.push(AsmLine::Label("__halt".to_string()));
        self.program.push(AsmLine::Instr(AsmInstr::SetPage("__halt".to_string())));
        self.program.push(AsmLine::Instr(AsmInstr::Goto("__halt".to_string())));
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
            self.program.push(AsmLine::Label(block_label(&name, block.id)));
            for instr in &block.instructions {
                self.emit_instr(function, instr, diagnostics);
            }
            self.emit_terminator(function, &block.terminator, diagnostics);
        }
    }

    /// Lowers a single IR instruction into PIC16 assembly.
    fn emit_instr(&mut self, function: &IrFunction, instr: &IrInstr, diagnostics: &mut DiagnosticBag) {
        match instr {
            IrInstr::Copy { dst, src } => {
                let ty = function.temp_types[*dst];
                self.copy_operand_to_temp(function.symbol, *src, ty, *dst);
            }
            IrInstr::AddrOf { dst, symbol } => {
                let dst_ty = function.temp_types[*dst];
                self.emit_address_of_symbol(function.symbol, *symbol, dst_ty, *dst);
            }
            IrInstr::Cast { dst, kind, src } => {
                let dst_ty = function.temp_types[*dst];
                self.emit_cast(function.symbol, *src, *kind, dst_ty, *dst);
            }
            IrInstr::Unary { dst, op, src } => {
                let dst_ty = function.temp_types[*dst];
                match op {
                    UnaryOp::Negate => self.emit_negate(function.symbol, *src, dst_ty, *dst),
                    UnaryOp::BitwiseNot => {
                        self.emit_per_byte_unary(function.symbol, *src, dst_ty, *dst, |this, sym, operand, ty, byte| {
                            this.load_operand_byte_to_w(sym, operand, ty, byte);
                            this.program.push(AsmLine::Instr(AsmInstr::Xorlw(0xFF)));
                        });
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
                    BinaryOp::Add => self.emit_add(function.symbol, *lhs, *rhs, dst_ty, *dst),
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
            IrInstr::Call {
                dst,
                function: callee,
                args,
            } => {
                self.emit_call(function, *callee, args, *dst, diagnostics);
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
            IrTerminator::Jump(target) => self.branch_to_label(&block_label(fn_name, *target)),
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
        if function.is_interrupt {
            diagnostics.error(
                "backend",
                None,
                format!(
                    "interrupt handler `{}` reached normal-call lowering for `{}`",
                    self.symbol_name(function.symbol),
                    self.symbol_name(callee)
                ),
                Some("phase 6 forbids normal function calls inside ISRs".to_string()),
            );
            if let Some(dst) = dst {
                self.clear_temp(function.symbol, dst, function.temp_types[dst]);
            }
            return;
        }

        let callee_symbol = &self.typed_program.symbols[callee];
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
        self.program.push(AsmLine::Instr(AsmInstr::SetPage(label.clone())));
        self.program.push(AsmLine::Instr(AsmInstr::Call(label)));

        let arg_bytes = self.function_arg_bytes(callee);
        if arg_bytes != 0 {
            self.add_immediate_to_pair(self.layout.helpers.stack_ptr, negate_u16(arg_bytes));
        }

        if let Some(dst) = dst {
            let dst_ty = function.temp_types[dst];
            self.store_w_to_temp_byte(function.symbol, dst, 0);
            if dst_ty.byte_width() == 2 {
                self.load_addr_to_w(self.layout.helpers.return_high);
                self.store_w_to_temp_byte(function.symbol, dst, 1);
            }
        }
    }

    /// Places a return operand into the Phase 4 return convention locations.
    fn emit_return_value(&mut self, function_symbol: SymbolId, value: Operand, return_ty: Type) {
        if return_ty.byte_width() == 2 {
            self.load_operand_byte_to_w(function_symbol, value, return_ty, 1);
            self.store_w_to_addr(self.layout.helpers.return_high);
        }
        self.load_operand_byte_to_w(function_symbol, value, return_ty, 0);
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
                        self.branch_to_label(targets.then_label);
                    } else {
                        self.branch_to_label(targets.else_label);
                    }
                    return;
                }

                match op {
                    BinaryOp::Equal => self.emit_equality_branch(
                        function.symbol,
                        *lhs,
                        *rhs,
                        *ty,
                        false,
                        targets,
                    ),
                    BinaryOp::NotEqual => self.emit_equality_branch(
                        function.symbol,
                        *lhs,
                        *rhs,
                        *ty,
                        true,
                        targets,
                    ),
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
                        self.branch_to_label(targets.else_label);
                    }
                }
            }
        }
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
                self.branch_to_label(then_label);
            } else {
                self.branch_to_label(else_label);
            }
            return;
        }

        if ty.byte_width() == 1 {
            self.load_operand_byte_to_w(function_symbol, value, ty, 0);
            self.branch_on_status_zero(false, then_label, else_label);
            return;
        }

        self.load_operand_byte_to_w(function_symbol, value, ty, 0);
        self.program.push(AsmLine::Instr(AsmInstr::Btfss {
            f: low7(STATUS_ADDR),
            b: STATUS_Z_BIT,
        }));
        self.branch_to_label(then_label);
        self.load_operand_byte_to_w(function_symbol, value, ty, 1);
        self.program.push(AsmLine::Instr(AsmInstr::Btfss {
            f: low7(STATUS_ADDR),
            b: STATUS_Z_BIT,
        }));
        self.branch_to_label(then_label);
        self.branch_to_label(else_label);
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
        if ty.byte_width() == 1 {
            self.compare_byte(function_symbol, lhs, rhs, ty, 0);
            self.branch_on_status_zero(!invert, targets.then_label, targets.else_label);
            return;
        }

        self.compare_byte(function_symbol, lhs, rhs, ty, 1);
        if invert {
            self.program.push(AsmLine::Instr(AsmInstr::Btfss {
                f: low7(STATUS_ADDR),
                b: STATUS_Z_BIT,
            }));
            self.branch_to_label(targets.then_label);
        } else {
            self.program.push(AsmLine::Instr(AsmInstr::Btfss {
                f: low7(STATUS_ADDR),
                b: STATUS_Z_BIT,
            }));
            self.branch_to_label(targets.else_label);
        }

        self.compare_byte(function_symbol, lhs, rhs, ty, 0);
        self.branch_on_status_zero(!invert, targets.then_label, targets.else_label);
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
        if ty.byte_width() == 1 {
            self.compare_byte(function_symbol, lhs, rhs, ty, 0);
            self.branch_on_unsigned_result(op, targets.then_label, targets.else_label);
            return;
        }

        let low_label = self.unique_label("cmp_low");
        self.compare_byte(function_symbol, lhs, rhs, ty, 1);
        self.program.push(AsmLine::Instr(AsmInstr::Btfsc {
            f: low7(STATUS_ADDR),
            b: STATUS_Z_BIT,
        }));
        self.branch_to_label(&low_label);
        self.branch_on_unsigned_result(op, targets.then_label, targets.else_label);
        self.program.push(AsmLine::Label(low_label));
        self.compare_byte(function_symbol, lhs, rhs, ty, 0);
        self.branch_on_unsigned_result(op, targets.then_label, targets.else_label);
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
        self.program.push(AsmLine::Instr(AsmInstr::Btfss {
            f: low7(self.layout.helpers.scratch1),
            b: 7,
        }));
        self.branch_to_label(&same_sign);

        match op {
            BinaryOp::Less | BinaryOp::LessEqual => {
                self.program.push(AsmLine::Instr(AsmInstr::Btfsc {
                    f: low7(self.layout.helpers.scratch0),
                    b: 7,
                }));
                self.branch_to_label(targets.then_label);
                self.branch_to_label(targets.else_label);
            }
            BinaryOp::Greater | BinaryOp::GreaterEqual => {
                self.program.push(AsmLine::Instr(AsmInstr::Btfsc {
                    f: low7(self.layout.helpers.scratch0),
                    b: 7,
                }));
                self.branch_to_label(targets.else_label);
                self.branch_to_label(targets.then_label);
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
                self.program.push(AsmLine::Instr(AsmInstr::Btfss {
                    f: low7(STATUS_ADDR),
                    b: STATUS_C_BIT,
                }));
                self.branch_to_label(then_label);
                self.branch_to_label(else_label);
            }
            BinaryOp::LessEqual => {
                self.program.push(AsmLine::Instr(AsmInstr::Btfss {
                    f: low7(STATUS_ADDR),
                    b: STATUS_C_BIT,
                }));
                self.branch_to_label(then_label);
                self.program.push(AsmLine::Instr(AsmInstr::Btfsc {
                    f: low7(STATUS_ADDR),
                    b: STATUS_Z_BIT,
                }));
                self.branch_to_label(then_label);
                self.branch_to_label(else_label);
            }
            BinaryOp::Greater => {
                self.program.push(AsmLine::Instr(AsmInstr::Btfss {
                    f: low7(STATUS_ADDR),
                    b: STATUS_C_BIT,
                }));
                self.branch_to_label(else_label);
                self.program.push(AsmLine::Instr(AsmInstr::Btfsc {
                    f: low7(STATUS_ADDR),
                    b: STATUS_Z_BIT,
                }));
                self.branch_to_label(else_label);
                self.branch_to_label(then_label);
            }
            BinaryOp::GreaterEqual => {
                self.program.push(AsmLine::Instr(AsmInstr::Btfsc {
                    f: low7(STATUS_ADDR),
                    b: STATUS_C_BIT,
                }));
                self.branch_to_label(then_label);
                self.branch_to_label(else_label);
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

    /// Emits 8-bit or 16-bit addition with explicit carry propagation.
    fn emit_add(&mut self, function_symbol: SymbolId, lhs: Operand, rhs: Operand, ty: Type, dst_temp: usize) {
        self.load_operand_byte_to_w(function_symbol, lhs, ty, 0);
        self.store_w_to_addr(self.layout.helpers.scratch0);
        self.load_operand_byte_to_w(function_symbol, rhs, ty, 0);
        self.select_bank(self.layout.helpers.scratch0);
        self.program.push(AsmLine::Instr(AsmInstr::Addwf {
            f: low7(self.layout.helpers.scratch0),
            d: Dest::W,
        }));
        self.store_w_to_temp_byte(function_symbol, dst_temp, 0);

        if ty.byte_width() == 2 {
            self.load_operand_byte_to_w(function_symbol, lhs, ty, 1);
            self.program.push(AsmLine::Instr(AsmInstr::Btfsc {
                f: low7(STATUS_ADDR),
                b: STATUS_C_BIT,
            }));
            self.program.push(AsmLine::Instr(AsmInstr::Addlw(1)));
            self.store_w_to_addr(self.layout.helpers.scratch0);
            self.load_operand_byte_to_w(function_symbol, rhs, ty, 1);
            self.select_bank(self.layout.helpers.scratch0);
            self.program.push(AsmLine::Instr(AsmInstr::Addwf {
                f: low7(self.layout.helpers.scratch0),
                d: Dest::W,
            }));
            self.store_w_to_temp_byte(function_symbol, dst_temp, 1);
        }
    }

    /// Emits 8-bit or 16-bit subtraction with explicit borrow propagation.
    fn emit_sub(&mut self, function_symbol: SymbolId, lhs: Operand, rhs: Operand, ty: Type, dst_temp: usize) {
        self.load_operand_byte_to_w(function_symbol, lhs, ty, 0);
        self.store_w_to_addr(self.layout.helpers.scratch0);
        self.load_operand_byte_to_w(function_symbol, rhs, ty, 0);
        self.select_bank(self.layout.helpers.scratch0);
        self.program.push(AsmLine::Instr(AsmInstr::Subwf {
            f: low7(self.layout.helpers.scratch0),
            d: Dest::W,
        }));
        self.store_w_to_temp_byte(function_symbol, dst_temp, 0);

        if ty.byte_width() == 2 {
            self.load_operand_byte_to_w(function_symbol, lhs, ty, 1);
            self.program.push(AsmLine::Instr(AsmInstr::Btfss {
                f: low7(STATUS_ADDR),
                b: STATUS_C_BIT,
            }));
            self.program.push(AsmLine::Instr(AsmInstr::Addlw(0xFF)));
            self.store_w_to_addr(self.layout.helpers.scratch0);
            self.load_operand_byte_to_w(function_symbol, rhs, ty, 1);
            self.select_bank(self.layout.helpers.scratch0);
            self.program.push(AsmLine::Instr(AsmInstr::Subwf {
                f: low7(self.layout.helpers.scratch0),
                d: Dest::W,
            }));
            self.store_w_to_temp_byte(function_symbol, dst_temp, 1);
        }
    }

    /// Emits two's-complement negation for the requested integer width.
    fn emit_negate(&mut self, function_symbol: SymbolId, src: Operand, ty: Type, dst_temp: usize) {
        self.clear_addr(self.layout.helpers.scratch0);
        self.load_operand_byte_to_w(function_symbol, src, ty, 0);
        self.select_bank(self.layout.helpers.scratch0);
        self.program.push(AsmLine::Instr(AsmInstr::Subwf {
            f: low7(self.layout.helpers.scratch0),
            d: Dest::W,
        }));
        self.store_w_to_temp_byte(function_symbol, dst_temp, 0);

        if ty.byte_width() == 2 {
            self.clear_addr(self.layout.helpers.scratch0);
            self.load_operand_byte_to_w(function_symbol, src, ty, 1);
            self.program.push(AsmLine::Instr(AsmInstr::Btfss {
                f: low7(STATUS_ADDR),
                b: STATUS_C_BIT,
            }));
            self.program.push(AsmLine::Instr(AsmInstr::Addlw(1)));
            self.select_bank(self.layout.helpers.scratch0);
            self.program.push(AsmLine::Instr(AsmInstr::Subwf {
                f: low7(self.layout.helpers.scratch0),
                d: Dest::W,
            }));
            self.store_w_to_temp_byte(function_symbol, dst_temp, 1);
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
                    self.emit_constant_shift(function_symbol, lhs, ty, dst_temp, count, op == BinaryOp::ShiftRight);
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
            let mask_byte = if byte == 0 {
                low_byte(mask, ty)
            } else {
                high_byte(mask, ty)
            };
            if mask_byte != 0xFF {
                self.program.push(AsmLine::Instr(AsmInstr::Andlw(mask_byte)));
            }
            self.store_w_to_temp_byte(function_symbol, dst_temp, byte);
        }
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
        self.push_operand(function_symbol, lhs, info.operand_ty);
        self.push_operand(function_symbol, rhs, info.operand_ty);
        self.program
            .push(AsmLine::Instr(AsmInstr::SetPage(info.label.to_string())));
        self.program
            .push(AsmLine::Instr(AsmInstr::Call(info.label.to_string())));
        self.add_immediate_to_pair(self.layout.helpers.stack_ptr, negate_u16(info.arg_bytes));
        self.store_w_to_temp_byte(function_symbol, dst_temp, 0);
        if ty.byte_width() == 2 {
            self.load_addr_to_w(self.layout.helpers.return_high);
            self.store_w_to_temp_byte(function_symbol, dst_temp, 1);
        }
    }

    /// Shifts one frame-resident scalar left by one bit using PIC16 rotate-through-carry.
    fn shift_current_frame_value_left(&mut self, offset: u16, ty: Type) {
        self.program.push(AsmLine::Instr(AsmInstr::Bcf {
            f: low7(STATUS_ADDR),
            b: STATUS_C_BIT,
        }));
        for byte in 0..ty.byte_width() {
            self.prepare_pointer_from_pair(self.layout.helpers.frame_ptr, offset + byte as u16);
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
            self.prepare_pointer_from_pair(self.layout.helpers.frame_ptr, offset + byte as u16);
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
            self.prepare_pointer_from_pair(self.layout.helpers.frame_ptr, offset + byte as u16);
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
        kind: CastKind,
        dst_ty: Type,
        dst_temp: usize,
    ) {
        match kind {
            CastKind::Bitcast => self.copy_operand_to_temp(function_symbol, src, dst_ty, dst_temp),
            CastKind::Truncate => {
                self.load_operand_byte_to_w(function_symbol, src, dst_ty, 0);
                self.store_w_to_temp_byte(function_symbol, dst_temp, 0);
            }
            CastKind::ZeroExtend => {
                self.load_operand_byte_to_w(function_symbol, src, Type::new(ScalarType::U8), 0);
                self.store_w_to_temp_byte(function_symbol, dst_temp, 0);
                if dst_ty.byte_width() == 2 {
                    self.emit_const_to_w(0);
                    self.store_w_to_temp_byte(function_symbol, dst_temp, 1);
                }
            }
            CastKind::SignExtend => {
                self.load_operand_byte_to_w(function_symbol, src, Type::new(ScalarType::U8), 0);
                self.store_w_to_temp_byte(function_symbol, dst_temp, 0);
                if dst_ty.byte_width() == 2 {
                    self.store_w_to_addr(self.layout.helpers.scratch0);
                    let positive = self.unique_label("sext_pos");
                    let end = self.unique_label("sext_end");
                    self.select_bank(self.layout.helpers.scratch0);
                    self.program.push(AsmLine::Instr(AsmInstr::Btfss {
                        f: low7(self.layout.helpers.scratch0),
                        b: 7,
                    }));
                    self.branch_to_label(&positive);
                    self.emit_const_to_w(0xFF);
                    self.store_w_to_temp_byte(function_symbol, dst_temp, 1);
                    self.branch_to_label(&end);
                    self.program.push(AsmLine::Label(positive));
                    self.emit_const_to_w(0);
                    self.store_w_to_temp_byte(function_symbol, dst_temp, 1);
                    self.program.push(AsmLine::Label(end));
                }
            }
        }
    }

    /// Copies an operand into one frame-scoped temporary slot.
    fn copy_operand_to_temp(&mut self, function_symbol: SymbolId, src: Operand, ty: Type, temp: usize) {
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
            SymbolStorage::Absolute(base) => self.copy_operand_to_slot(function_symbol, src, ty, base),
            SymbolStorage::Frame(offset) => {
                for byte in 0..ty.byte_width() {
                    self.load_operand_byte_to_w(function_symbol, src, ty, byte);
                    self.store_w_to_frame_byte(function_symbol, offset + byte as u16);
                }
            }
        }
    }

    /// Copies an operand into any RAM slot, respecting 8-bit or 16-bit width.
    fn copy_operand_to_slot(&mut self, function_symbol: SymbolId, src: Operand, ty: Type, dst_base: u16) {
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
                self.store_w_to_temp_byte(function_symbol, dst_temp, 0);
                if dst_ty.byte_width() == 2 {
                    self.load_addr_to_w(self.layout.helpers.scratch1);
                    self.store_w_to_temp_byte(function_symbol, dst_temp, 1);
                }
            }
        }
    }

    /// Emits one function prologue that establishes the Phase 4 software frame pointer.
    fn emit_prologue(&mut self, function_symbol: SymbolId) {
        let arg_bytes = self.frame_layout(function_symbol).arg_bytes;
        let frame_bytes = self.frame_layout(function_symbol).frame_bytes;
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
        self.load_addr_to_w(self.layout.helpers.scratch0);
        self.store_w_to_shared_addr(ctx.scratch0);
        self.load_addr_to_w(self.layout.helpers.scratch1);
        self.store_w_to_shared_addr(ctx.scratch1);
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
        self.load_shared_addr_to_w(ctx.scratch0);
        self.store_w_to_addr(self.layout.helpers.scratch0);
        self.load_shared_addr_to_w(ctx.scratch1);
        self.store_w_to_addr(self.layout.helpers.scratch1);
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

    /// Pushes the current `W` byte to the stack top and advances `SP`.
    fn push_w(&mut self) {
        self.store_w_to_addr(self.layout.helpers.scratch0);
        self.prepare_pointer_from_pair(self.layout.helpers.stack_ptr, 0);
        self.load_addr_to_w(self.layout.helpers.scratch0);
        self.store_w_to_indirect();
        self.add_immediate_to_pair(self.layout.helpers.stack_ptr, 1);
    }

    /// Copies one 16-bit helper pair into another while applying a signed constant offset.
    fn copy_pair_with_signed_offset(
        &mut self,
        src: RegisterPair,
        dst: RegisterPair,
        delta: u16,
    ) {
        let delta_ty = Type::new(ScalarType::U16);
        self.load_addr_to_w(src.lo);
        self.program.push(AsmLine::Instr(AsmInstr::Addlw(low_byte(i64::from(delta), delta_ty))));
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
        self.prepare_pointer_from_pair(self.layout.helpers.frame_ptr, offset);
        self.load_indirect_to_w();
    }

    /// Stores `W` into one byte of the active frame at `FP + offset`.
    fn store_w_to_current_frame_byte(&mut self, offset: u16) {
        self.prepare_pointer_from_pair(self.layout.helpers.frame_ptr, offset);
        self.store_w_to_indirect();
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
        self.program.push(AsmLine::Instr(AsmInstr::Btfsc {
            f: low7(INDF_ADDR),
            b: bit,
        }));
        self.branch_to_label(set_label);
        self.branch_to_label(clear_label);
    }

    /// Clears one scalar slot that lives inside the active call frame.
    fn clear_current_frame_slot(&mut self, offset: u16, ty: Type) {
        for byte in 0..ty.byte_width() {
            self.emit_const_to_w(0);
            self.store_w_to_current_frame_byte(offset + byte as u16);
        }
    }

    /// Branches on whether one active-frame scalar value is zero or non-zero.
    fn emit_current_frame_nonzero_branch(&mut self, offset: u16, ty: Type, then_label: &str, else_label: &str) {
        if ty.byte_width() == 1 {
            self.load_current_frame_byte_to_w(offset);
            self.branch_on_status_zero(false, then_label, else_label);
            return;
        }

        self.load_current_frame_byte_to_w(offset);
        self.program.push(AsmLine::Instr(AsmInstr::Btfss {
            f: low7(STATUS_ADDR),
            b: STATUS_Z_BIT,
        }));
        self.branch_to_label(then_label);
        self.load_current_frame_byte_to_w(offset + 1);
        self.program.push(AsmLine::Instr(AsmInstr::Btfss {
            f: low7(STATUS_ADDR),
            b: STATUS_Z_BIT,
        }));
        self.branch_to_label(then_label);
        self.branch_to_label(else_label);
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
        if ty.byte_width() == 1 {
            self.compare_current_frame_byte(lhs_offset, rhs_offset, 0);
            self.program.push(AsmLine::Instr(AsmInstr::Btfsc {
                f: low7(STATUS_ADDR),
                b: STATUS_C_BIT,
            }));
            self.branch_to_label(ge_label);
            self.branch_to_label(lt_label);
            return;
        }

        let low_label = self.unique_label("rt_cmp_low");
        self.compare_current_frame_byte(lhs_offset, rhs_offset, 1);
        self.program.push(AsmLine::Instr(AsmInstr::Btfsc {
            f: low7(STATUS_ADDR),
            b: STATUS_Z_BIT,
        }));
        self.branch_to_label(&low_label);
        self.program.push(AsmLine::Instr(AsmInstr::Btfsc {
            f: low7(STATUS_ADDR),
            b: STATUS_C_BIT,
        }));
        self.branch_to_label(ge_label);
        self.branch_to_label(lt_label);
        self.program.push(AsmLine::Label(low_label));
        self.compare_current_frame_byte(lhs_offset, rhs_offset, 0);
        self.program.push(AsmLine::Instr(AsmInstr::Btfsc {
            f: low7(STATUS_ADDR),
            b: STATUS_C_BIT,
        }));
        self.branch_to_label(ge_label);
        self.branch_to_label(lt_label);
    }

    /// Adds one active-frame scalar into another slot in place.
    fn add_current_frame_value_into_slot(&mut self, src_offset: u16, dst_offset: u16, ty: Type) {
        self.load_current_frame_byte_to_w(src_offset);
        self.store_w_to_addr(self.layout.helpers.scratch0);
        self.load_current_frame_byte_to_w(dst_offset);
        self.select_bank(self.layout.helpers.scratch0);
        self.program.push(AsmLine::Instr(AsmInstr::Addwf {
            f: low7(self.layout.helpers.scratch0),
            d: Dest::W,
        }));
        self.store_w_to_current_frame_byte(dst_offset);

        if ty.byte_width() == 2 {
            self.load_current_frame_byte_to_w(src_offset + 1);
            self.program.push(AsmLine::Instr(AsmInstr::Btfsc {
                f: low7(STATUS_ADDR),
                b: STATUS_C_BIT,
            }));
            self.program.push(AsmLine::Instr(AsmInstr::Addlw(1)));
            self.store_w_to_addr(self.layout.helpers.scratch0);
            self.load_current_frame_byte_to_w(dst_offset + 1);
            self.select_bank(self.layout.helpers.scratch0);
            self.program.push(AsmLine::Instr(AsmInstr::Addwf {
                f: low7(self.layout.helpers.scratch0),
                d: Dest::W,
            }));
            self.store_w_to_current_frame_byte(dst_offset + 1);
        }
    }

    /// Subtracts one active-frame scalar from another slot in place.
    fn sub_current_frame_value_from_slot(&mut self, src_offset: u16, dst_offset: u16, ty: Type) {
        self.load_current_frame_byte_to_w(dst_offset);
        self.store_w_to_addr(self.layout.helpers.scratch0);
        self.load_current_frame_byte_to_w(src_offset);
        self.select_bank(self.layout.helpers.scratch0);
        self.program.push(AsmLine::Instr(AsmInstr::Subwf {
            f: low7(self.layout.helpers.scratch0),
            d: Dest::W,
        }));
        self.store_w_to_current_frame_byte(dst_offset);

        if ty.byte_width() == 2 {
            self.load_current_frame_byte_to_w(dst_offset + 1);
            self.program.push(AsmLine::Instr(AsmInstr::Btfss {
                f: low7(STATUS_ADDR),
                b: STATUS_C_BIT,
            }));
            self.program.push(AsmLine::Instr(AsmInstr::Addlw(0xFF)));
            self.store_w_to_addr(self.layout.helpers.scratch0);
            self.load_current_frame_byte_to_w(src_offset + 1);
            self.select_bank(self.layout.helpers.scratch0);
            self.program.push(AsmLine::Instr(AsmInstr::Subwf {
                f: low7(self.layout.helpers.scratch0),
                d: Dest::W,
            }));
            self.store_w_to_current_frame_byte(dst_offset + 1);
        }
    }

    /// Negates one active-frame scalar in place with two's-complement wrap semantics.
    fn negate_current_frame_value(&mut self, offset: u16, ty: Type) {
        self.clear_addr(self.layout.helpers.scratch0);
        self.load_current_frame_byte_to_w(offset);
        self.select_bank(self.layout.helpers.scratch0);
        self.program.push(AsmLine::Instr(AsmInstr::Subwf {
            f: low7(self.layout.helpers.scratch0),
            d: Dest::W,
        }));
        self.store_w_to_current_frame_byte(offset);

        if ty.byte_width() == 2 {
            self.clear_addr(self.layout.helpers.scratch0);
            self.load_current_frame_byte_to_w(offset + 1);
            self.program.push(AsmLine::Instr(AsmInstr::Btfss {
                f: low7(STATUS_ADDR),
                b: STATUS_C_BIT,
            }));
            self.program.push(AsmLine::Instr(AsmInstr::Addlw(1)));
            self.select_bank(self.layout.helpers.scratch0);
            self.program.push(AsmLine::Instr(AsmInstr::Subwf {
                f: low7(self.layout.helpers.scratch0),
                d: Dest::W,
            }));
            self.store_w_to_current_frame_byte(offset + 1);
        }
    }

    /// Decrements one active-frame scalar in place.
    fn decrement_current_frame_value(&mut self, offset: u16, ty: Type) {
        self.load_current_frame_byte_to_w(offset);
        self.program.push(AsmLine::Instr(AsmInstr::Addlw(0xFF)));
        self.store_w_to_current_frame_byte(offset);
        if ty.byte_width() == 2 {
            self.load_current_frame_byte_to_w(offset + 1);
            self.program.push(AsmLine::Instr(AsmInstr::Btfss {
                f: low7(STATUS_ADDR),
                b: STATUS_C_BIT,
            }));
            self.program.push(AsmLine::Instr(AsmInstr::Addlw(0xFF)));
            self.store_w_to_current_frame_byte(offset + 1);
        }
    }

    /// Places one active-frame scalar into the ABI return locations.
    fn emit_return_current_frame_value(&mut self, offset: u16, ty: Type) {
        if ty.byte_width() == 2 {
            self.load_current_frame_byte_to_w(offset + 1);
            self.store_w_to_addr(self.layout.helpers.return_high);
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
            self.program.push(AsmLine::Instr(AsmInstr::Addlw(byte_offset)));
        }
        self.store_w_to_addr(FSR_ADDR);

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
        self.program.push(AsmLine::Instr(AsmInstr::Movwf(low7(INDF_ADDR))));
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
                let byte = if byte_index == 0 {
                    low_byte(value, ty)
                } else {
                    high_byte(value, ty)
                };
                self.emit_const_to_w(byte);
            }
            Operand::Symbol(symbol) => match self.symbol_storage(symbol) {
                SymbolStorage::Absolute(base) => self.load_addr_to_w(base + byte_index as u16),
                SymbolStorage::Frame(offset) => {
                    self.load_frame_byte_to_w(function_symbol, offset + byte_index as u16)
                }
            },
            Operand::Temp(temp) => {
                self.load_frame_byte_to_w(function_symbol, self.temp_offset(function_symbol, temp) + byte_index as u16)
            }
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
        self.emit_const_to_w(low_byte(value, ty));
        self.store_w_to_addr(base);
        if ty.byte_width() == 2 {
            self.emit_const_to_w(high_byte(value, ty));
            self.store_w_to_addr(base + 1);
        }
    }

    /// Stores the current `W` value into a banked RAM address.
    fn store_w_to_addr(&mut self, addr: u16) {
        self.select_bank(addr);
        self.program.push(AsmLine::Instr(AsmInstr::Movwf(low7(addr))));
        if addr == STATUS_ADDR {
            self.current_bank = UNKNOWN_BANK;
        }
    }

    /// Stores the current `W` value into a mirrored/common address without bank selection.
    fn store_w_to_direct_addr(&mut self, addr: u16) {
        self.program.push(AsmLine::Instr(AsmInstr::Movwf(low7(addr))));
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
        self.program.push(AsmLine::Instr(AsmInstr::Clrf(low7(addr))));
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

    /// Branches based on the zero flag after a prior compare or test.
    fn branch_on_status_zero(&mut self, zero_means_true: bool, then_label: &str, else_label: &str) {
        let status = low7(STATUS_ADDR);
        self.program.push(AsmLine::Instr(if zero_means_true {
            AsmInstr::Btfss {
                f: status,
                b: STATUS_Z_BIT,
            }
        } else {
            AsmInstr::Btfsc {
                f: status,
                b: STATUS_Z_BIT,
            }
        }));
        self.branch_to_label(else_label);
        self.branch_to_label(then_label);
    }

    /// Emits a page-safe unconditional branch to a label.
    fn branch_to_label(&mut self, label: &str) {
        self.program.push(AsmLine::Instr(AsmInstr::SetPage(label.to_string())));
        self.program.push(AsmLine::Instr(AsmInstr::Goto(label.to_string())));
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
                self.program.push(AsmLine::Instr(AsmInstr::Bcf { f: status, b: 5 }));
            } else {
                self.program.push(AsmLine::Instr(AsmInstr::Bsf { f: status, b: 5 }));
            }
        }
        if self.current_bank == UNKNOWN_BANK || ((self.current_bank ^ bank) & 0x02) != 0 {
            if (bank & 0x02) == 0 {
                self.program.push(AsmLine::Instr(AsmInstr::Bcf { f: status, b: 6 }));
            } else {
                self.program.push(AsmLine::Instr(AsmInstr::Bsf { f: status, b: 6 }));
            }
        }
        self.current_bank = bank;
    }

    /// Emits every internal arithmetic helper that codegen marked as used.
    fn emit_runtime_helpers(&mut self) {
        let helpers = self.used_helpers.iter().copied().collect::<Vec<_>>();
        if !helpers.is_empty() {
            self.program.push(AsmLine::Comment(
                "runtime helpers (Phase 7 optimized section)".to_string(),
            ));
        }
        for helper in helpers {
            self.emit_runtime_helper(helper);
        }
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

        self.program.push(AsmLine::Label(info.label.to_string()));
        self.program.push(AsmLine::Comment(format!(
            "runtime helper {:?} args={} locals={} frame_bytes={}",
            helper, info.arg_bytes, info.local_bytes, info.frame_bytes
        )));
        self.current_bank = UNKNOWN_BANK;
        self.emit_runtime_prologue(info);

        match helper {
            RuntimeHelper::MulU8 | RuntimeHelper::MulU16 => {
                self.clear_current_frame_slot(work_offset, ty);
                self.emit_const_to_w(ty.bit_width() as u8);
                self.store_w_to_current_frame_byte(count_offset);
                self.emit_unsigned_mul_core(arg0_offset, arg1_offset, work_offset, count_offset, ty);
                self.emit_return_current_frame_value(work_offset, ty);
            }
            RuntimeHelper::MulI8 | RuntimeHelper::MulI16 => {
                self.clear_current_frame_slot(flag_offset, Type::new(ScalarType::U8));
                self.emit_runtime_negate_if_signed(arg0_offset, ty, flag_offset, 0x01);
                self.emit_runtime_negate_if_signed(arg1_offset, ty, flag_offset, 0x01);
                self.clear_current_frame_slot(work_offset, ty);
                self.emit_const_to_w(ty.bit_width() as u8);
                self.store_w_to_current_frame_byte(count_offset);
                self.emit_unsigned_mul_core(arg0_offset, arg1_offset, work_offset, count_offset, ty);

                let negate_label = self.unique_label("rt_mul_neg");
                let done_label = self.unique_label("rt_mul_done");
                self.branch_on_current_frame_bit(flag_offset, 0, &negate_label, &done_label);
                self.program.push(AsmLine::Label(negate_label));
                self.negate_current_frame_value(work_offset, ty);
                self.program.push(AsmLine::Label(done_label));
                self.emit_return_current_frame_value(work_offset, ty);
            }
            RuntimeHelper::DivU8
            | RuntimeHelper::DivU16
            | RuntimeHelper::ModU8
            | RuntimeHelper::ModU16 => {
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
                self.branch_to_label(&finish_label);
                self.program.push(AsmLine::Label(finish_label));
                let result_offset = if matches!(helper, RuntimeHelper::DivU8 | RuntimeHelper::DivU16) {
                    arg0_offset
                } else {
                    work_offset
                };
                self.emit_return_current_frame_value(result_offset, ty);
            }
            RuntimeHelper::DivI8
            | RuntimeHelper::DivI16
            | RuntimeHelper::ModI8
            | RuntimeHelper::ModI16 => {
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
                self.branch_to_label(&finish_label);
                self.program.push(AsmLine::Label(finish_label));

                if matches!(helper, RuntimeHelper::DivI8 | RuntimeHelper::DivI16) {
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
            | RuntimeHelper::ShrU8
            | RuntimeHelper::ShrI8
            | RuntimeHelper::ShrU16
            | RuntimeHelper::ShrI16 => {
                let loop_label = self.unique_label("rt_shift_loop");
                let body_label = self.unique_label("rt_shift_body");
                let done_label = self.unique_label("rt_shift_done");
                self.clamp_runtime_shift_count(arg1_offset, ty);
                self.program.push(AsmLine::Label(loop_label.clone()));
                self.emit_current_frame_nonzero_branch(arg1_offset, ty, &body_label, &done_label);
                self.program.push(AsmLine::Label(body_label));
                if matches!(helper, RuntimeHelper::Shl8 | RuntimeHelper::Shl16) {
                    self.shift_current_frame_value_left(arg0_offset, ty);
                } else {
                    self.shift_current_frame_value_right(
                        arg0_offset,
                        ty,
                        matches!(helper, RuntimeHelper::ShrI8 | RuntimeHelper::ShrI16),
                    );
                }
                self.decrement_current_frame_value(arg1_offset, ty);
                self.branch_to_label(&loop_label);
                self.program.push(AsmLine::Label(done_label));
                self.emit_return_current_frame_value(arg0_offset, ty);
            }
        }

        self.emit_runtime_epilogue(info);
        self.program.push(AsmLine::Instr(AsmInstr::Return));
    }

    /// Emits the common stack-first runtime-helper prologue.
    fn emit_runtime_prologue(&mut self, info: RuntimeHelperInfo) {
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
        self.emit_current_frame_nonzero_branch(count_offset, Type::new(ScalarType::U8), &body_label, &done_label);
        self.program.push(AsmLine::Label(body_label));
        self.branch_on_current_frame_bit(multiplier_offset, 0, &add_label, &next_label);
        self.program.push(AsmLine::Label(add_label));
        self.add_current_frame_value_into_slot(multiplicand_offset, result_offset, ty);
        self.program.push(AsmLine::Label(next_label));
        self.shift_current_frame_value_left(multiplicand_offset, ty);
        self.shift_current_frame_value_right(multiplier_offset, ty, false);
        self.decrement_current_frame_value(count_offset, Type::new(ScalarType::U8));
        self.branch_to_label(&loop_label);
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
        self.emit_current_frame_nonzero_branch(count_offset, Type::new(ScalarType::U8), &body_label, finish_label);
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
        self.branch_to_label(&loop_label);
    }

    /// Negates a signed arg in place and toggles one helper flag when the sign bit was set.
    fn emit_runtime_negate_if_signed(&mut self, offset: u16, ty: Type, flag_offset: u16, flag_mask: u8) {
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
        self.program.push(AsmLine::Instr(AsmInstr::Xorlw(flag_mask)));
        self.store_w_to_current_frame_byte(flag_offset);
        self.negate_current_frame_value(offset, ty);
        self.program.push(AsmLine::Label(done_label));
    }

    /// Sets one helper flag when a signed arg carries a negative sign bit.
    fn emit_runtime_set_flag_if_signed(&mut self, offset: u16, ty: Type, flag_offset: u16, flag_mask: u8) {
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
        self.program.push(AsmLine::Instr(AsmInstr::Iorlw(flag_mask)));
        self.store_w_to_current_frame_byte(flag_offset);
        self.program.push(AsmLine::Label(done_label));
    }

    /// Clamps a dynamic shift count to the operand bit width to avoid unbounded helper loops.
    fn clamp_runtime_shift_count(&mut self, offset: u16, ty: Type) {
        let clamp_label = self.unique_label("rt_shift_clamp");
        let done_label = self.unique_label("rt_shift_clamp_done");
        let width = ty.bit_width() as u8;

        if ty.byte_width() == 2 {
            self.emit_current_frame_nonzero_branch(offset + 1, Type::new(ScalarType::U8), &clamp_label, &done_label);
            self.program.push(AsmLine::Label(done_label.clone()));
        }

        self.load_current_frame_byte_to_w(offset);
        self.store_w_to_addr(self.layout.helpers.scratch0);
        self.emit_const_to_w(width);
        self.select_bank(self.layout.helpers.scratch0);
        self.program.push(AsmLine::Instr(AsmInstr::Subwf {
            f: low7(self.layout.helpers.scratch0),
            d: Dest::W,
        }));
        self.program.push(AsmLine::Instr(AsmInstr::Btfsc {
            f: low7(STATUS_ADDR),
            b: STATUS_C_BIT,
        }));
        self.branch_to_label(&clamp_label);
        if ty.byte_width() == 2 {
            let final_done = self.unique_label("rt_shift_clamp_done2");
            self.branch_to_label(&final_done);
            self.program.push(AsmLine::Label(clamp_label));
            self.emit_const_to_w(width);
            self.store_w_to_current_frame_byte(offset);
            self.emit_const_to_w(0);
            self.store_w_to_current_frame_byte(offset + 1);
            self.program.push(AsmLine::Label(final_done));
            return;
        }

        self.branch_to_label(&done_label);
        self.program.push(AsmLine::Label(clamp_label));
        self.emit_const_to_w(width);
        self.store_w_to_current_frame_byte(offset);
        self.program.push(AsmLine::Label(done_label));
    }

    /// Returns the storage classification assigned to one source-level symbol.
    fn symbol_storage(&self, symbol: SymbolId) -> SymbolStorage {
        self.layout.symbol_storage[&symbol]
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

/// Builds the assembly label used for one function-local basic block.
fn block_label(function_name: &str, block: usize) -> String {
    format!("fn_{function_name}_b{block}")
}

/// Computes the worst-case software-stack depth across normal code plus one interrupt frame.
fn compute_max_stack_depth_with_interrupts(
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
                    IrInstr::Call { function: callee, .. } => callees.push(*callee),
                    IrInstr::Binary { dst, op, .. } => {
                        if let Some(helper) = binary_helper(*op, function.temp_types[*dst]) {
                            let info = helper.info();
                            helper_depth = helper_depth.max(info.arg_bytes + info.frame_bytes);
                        }
                    }
                    _ => {}
                }
            }
        }
        calls.insert(function.symbol, callees);
        helper_depths.insert(function.symbol, helper_depth);
    }

    let mut memo = BTreeMap::new();
    let normal_max = calls
        .keys()
        .copied()
        .map(|symbol| compute_function_stack_depth(symbol, &calls, &helper_depths, frames, &mut memo))
        .max()
        .unwrap_or(0);
    let interrupt_extra = ir_program
        .functions
        .iter()
        .filter(|function| function.is_interrupt)
        .map(|function| frames.get(&function.symbol).map_or(0, |frame| frame.frame_bytes))
        .max()
        .unwrap_or(0);
    normal_max + interrupt_extra
}

#[cfg(test)]
/// Compatibility wrapper for tests that do not model interrupts explicitly.
fn compute_max_stack_depth(ir_program: &IrProgram, frames: &BTreeMap<SymbolId, FrameLayout>) -> u16 {
    compute_max_stack_depth_with_interrupts(ir_program, frames)
}

/// Computes the worst-case stack usage while one function is active.
fn compute_function_stack_depth(
    symbol: SymbolId,
    calls: &BTreeMap<SymbolId, Vec<SymbolId>>,
    helper_depths: &BTreeMap<SymbolId, u16>,
    frames: &BTreeMap<SymbolId, FrameLayout>,
    memo: &mut BTreeMap<SymbolId, u16>,
) -> u16 {
    if let Some(depth) = memo.get(&symbol).copied() {
        return depth;
    }

    let own = frames.get(&symbol).map_or(0, |frame| frame.frame_bytes);
    let nested = calls
        .get(&symbol)
        .into_iter()
        .flatten()
        .map(|callee| {
            let arg_bytes = frames.get(callee).map_or(0, |frame| frame.arg_bytes);
            arg_bytes + compute_function_stack_depth(*callee, calls, helper_depths, frames, memo)
        })
        .max()
        .unwrap_or(0);
    let depth = own + nested.max(helper_depths.get(&symbol).copied().unwrap_or(0));
    memo.insert(symbol, depth);
    depth
}

/// Builds the final map file from encoded labels and allocated data symbols.
fn build_map(
    typed_program: &TypedProgram,
    layout: &StorageLayout,
    labels: &BTreeMap<String, u16>,
) -> MapFile {
    let mut code_symbols = labels
        .iter()
        .map(|(name, addr)| (name.clone(), *addr))
        .collect::<Vec<_>>();
    code_symbols.sort_by_key(|(_, addr)| *addr);

    let mut data_symbols = typed_program
        .symbols
        .iter()
        .filter(|symbol| symbol.kind != SymbolKind::Function)
        .filter_map(|symbol| {
            match layout.symbol_storage.get(&symbol.id).copied() {
                Some(SymbolStorage::Absolute(addr)) => Some((format_data_symbol_name(symbol), addr)),
                Some(SymbolStorage::Frame(_)) | None => None,
            }
        })
        .collect::<Vec<_>>();
    data_symbols.extend([
        ("__abi.stack_ptr.lo".to_string(), layout.helpers.stack_ptr.lo),
        ("__abi.stack_ptr.hi".to_string(), layout.helpers.stack_ptr.hi),
        ("__abi.frame_ptr.lo".to_string(), layout.helpers.frame_ptr.lo),
        ("__abi.frame_ptr.hi".to_string(), layout.helpers.frame_ptr.hi),
        ("__abi.return_high".to_string(), layout.helpers.return_high),
        ("__abi.scratch0".to_string(), layout.helpers.scratch0),
        ("__abi.scratch1".to_string(), layout.helpers.scratch1),
        ("__stack.base".to_string(), layout.stack_base),
        ("__stack.end".to_string(), layout.stack_end),
    ]);
    if let Some(interrupt) = layout.interrupt {
        data_symbols.extend([
            ("__isr_ctx.w".to_string(), interrupt.w),
            ("__isr_ctx.status".to_string(), interrupt.status),
            ("__isr_ctx.pclath".to_string(), interrupt.pclath),
            ("__isr_ctx.fsr".to_string(), interrupt.fsr),
            ("__isr_ctx.return_high".to_string(), interrupt.return_high),
            ("__isr_ctx.scratch0".to_string(), interrupt.scratch0),
            ("__isr_ctx.scratch1".to_string(), interrupt.scratch1),
            ("__isr_ctx.stack_ptr.lo".to_string(), interrupt.stack_ptr.lo),
            ("__isr_ctx.stack_ptr.hi".to_string(), interrupt.stack_ptr.hi),
            ("__isr_ctx.frame_ptr.lo".to_string(), interrupt.frame_ptr.lo),
            ("__isr_ctx.frame_ptr.hi".to_string(), interrupt.frame_ptr.hi),
        ]);
    }
    data_symbols.sort_by_key(|(_, addr)| *addr);

    MapFile {
        code_symbols,
        data_symbols,
    }
}

/// Formats one data symbol with simple qualifiers that help map/listing readers.
fn format_data_symbol_name(symbol: &Symbol) -> String {
    let mut tags = Vec::new();
    if symbol.ty.qualifiers.is_const {
        tags.push("const");
    }
    match (symbol.kind, symbol.storage_class) {
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

/// Evaluates a constant typed expression for startup initialization purposes.
fn eval_const_expr(expr: &TypedExpr) -> i64 {
    let target_ty = expr.ty;
    match &expr.kind {
        TypedExprKind::IntLiteral(value) => *value,
        TypedExprKind::Unary { op, expr } => eval_unary(*op, eval_const_expr(expr), expr.ty, target_ty),
        TypedExprKind::Binary { op, lhs, rhs } => {
            eval_binary(*op, eval_const_expr(lhs), eval_const_expr(rhs), lhs.ty, target_ty)
        }
        TypedExprKind::Cast {
            kind,
            expr: value_expr,
        } => {
            let value = eval_const_expr(value_expr);
            match kind {
                CastKind::ZeroExtend | CastKind::Truncate | CastKind::Bitcast => {
                    normalize_value(value, target_ty)
                }
                CastKind::SignExtend => normalize_value(signed_value(value, value_expr.ty), target_ty),
            }
        }
        TypedExprKind::Assign { .. }
        | TypedExprKind::Call { .. }
        | TypedExprKind::ArrayDecay(_)
        | TypedExprKind::AddressOf(_)
        | TypedExprKind::Deref(_)
        | TypedExprKind::Symbol(_) => 0,
    }
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
        compile_program, compute_max_stack_depth, low7, CodegenContext, FrameLayout, StorageAllocator,
    };
    use crate::backend::pic16::midrange14::asm::{AsmInstr, AsmLine};
    use crate::backend::pic16::devices::DeviceRegistry;
    use crate::frontend::ast::BinaryOp;
    use crate::diagnostics::{DiagnosticBag, WarningProfile};
    use crate::frontend::semantic::{Symbol, SymbolKind, TypedFunction, TypedGlobal, TypedProgram};
    use crate::frontend::types::{ScalarType, StorageClass, Type};
    use crate::ir::model::{IrBlock, IrFunction, IrInstr, IrProgram, IrTerminator, Operand};
    use crate::common::source::Span;
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
        let output = compile_program(target, &program, &ir, &mut diagnostics).expect("backend");

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
                            args: vec![Operand::Constant(1), Operand::Constant(2), Operand::Constant(3)],
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
                            args: vec![Operand::Constant(1), Operand::Constant(2), Operand::Constant(3)],
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
        let output = compile_program(target, &program, &ir, &mut diagnostics).expect("backend");

        assert!(!diagnostics.has_errors());
        let asm = output.program.render();
        assert!(asm.contains("stack base="));
        assert!(asm.contains("call fn_sum3"));
        assert!(output.map.data_symbols.iter().any(|(name, _)| name == "__abi.stack_ptr.lo"));
        assert!(output.map.data_symbols.iter().any(|(name, _)| name == "__stack.base"));
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
        let mut codegen = CodegenContext::new(target, &program, &layout);
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
            symbols: vec![symbol(0, "main", Type::new(ScalarType::Void), SymbolKind::Function)],
            globals: Vec::new(),
            functions: vec![TypedFunction {
                symbol: 0,
                params: Vec::new(),
                locals: Vec::new(),
                body: None,
                return_type: Type::new(ScalarType::Void),
                span: Span::new(0, 0),
            }],
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
        let output = compile_program(target, &program, &ir, &mut diagnostics).expect("backend");

        assert!(!diagnostics.has_errors());
        let asm = output.program.render();
        assert!(asm.contains("call __rt_mul_u16"));
        assert!(asm.contains("__rt_mul_u16:"));
        assert!(output.map.code_symbols.iter().any(|(name, _)| name == "__rt_mul_u16"));
    }

    #[test]
    /// Verifies constant-count shifts lower inline without pulling in the dynamic helper path.
    fn phase_five_constant_shift_stays_inline() {
        let registry = DeviceRegistry::new();
        let target = registry.device("pic16f628a").expect("device");
        let u16_ty = Type::new(ScalarType::U16);
        let program = TypedProgram {
            symbols: vec![symbol(0, "main", Type::new(ScalarType::Void), SymbolKind::Function)],
            globals: Vec::new(),
            functions: vec![TypedFunction {
                symbol: 0,
                params: Vec::new(),
                locals: Vec::new(),
                body: None,
                return_type: Type::new(ScalarType::Void),
                span: Span::new(0, 0),
            }],
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
        let output = compile_program(target, &program, &ir, &mut diagnostics).expect("backend");

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
            symbols: vec![symbol(0, "main", Type::new(ScalarType::Void), SymbolKind::Function)],
            globals: Vec::new(),
            functions: vec![TypedFunction {
                symbol: 0,
                params: Vec::new(),
                locals: Vec::new(),
                body: None,
                return_type: Type::new(ScalarType::Void),
                span: Span::new(0, 0),
            }],
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
        let output = compile_program(target, &program, &ir, &mut diagnostics).expect("backend");

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
        let output = compile_program(target, &program, &ir, &mut diagnostics).expect("backend");

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
        assert_eq!(output.words[&target.vectors.reset], 0x2800 | (reset_dispatch & 0x07FF));
        assert_eq!(
            output.words[&target.vectors.interrupt],
            0x2800 | (interrupt_dispatch & 0x07FF)
        );
        assert!(output
            .map
            .data_symbols
            .iter()
            .any(|(name, _)| name == "__isr_ctx.w"));
        assert!(output
            .map
            .data_symbols
            .iter()
            .any(|(name, _)| name == "__isr_ctx.stack_ptr.lo"));
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
