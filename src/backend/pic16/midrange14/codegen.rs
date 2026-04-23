use std::collections::BTreeMap;

use crate::backend::pic16::devices::{MemoryRange, TargetDevice};
use crate::common::integer::{
    compare_rel, eval_binary, eval_unary, high_byte, low_byte, normalize_value, signed_value,
};
use crate::diagnostics::DiagnosticBag;
use crate::frontend::ast::{BinaryOp, UnaryOp};
use crate::frontend::semantic::{SymbolId, SymbolKind, TypedExpr, TypedExprKind, TypedProgram};
use crate::frontend::types::{CastKind, ScalarType, Type};
use crate::ir::model::{IrCondition, IrFunction, IrInstr, IrProgram, IrTerminator, Operand};
use crate::linker::map::MapFile;

use super::asm::{AsmInstr, AsmLine, AsmProgram, Dest};
use super::encoder::encode_program;

const STATUS_ADDR: u16 = 0x03;
const STATUS_C_BIT: u8 = 0;
const STATUS_Z_BIT: u8 = 2;
const STATUS_IRP_BIT: u8 = 7;
const INDF_ADDR: u16 = 0x00;
const FSR_ADDR: u16 = 0x04;

#[derive(Debug)]
pub struct BackendOutput {
    pub program: AsmProgram,
    pub words: BTreeMap<u16, u16>,
    pub map: MapFile,
}

#[derive(Clone, Copy, Debug)]
struct RegisterPair {
    lo: u16,
    hi: u16,
}

#[derive(Clone, Copy, Debug)]
struct HelperRegisters {
    arg0: RegisterPair,
    arg1: RegisterPair,
    return_high: u16,
    scratch0: u16,
    scratch1: u16,
}

#[derive(Clone, Copy, Debug)]
struct BranchTargets<'a> {
    then_label: &'a str,
    else_label: &'a str,
}

#[derive(Debug)]
struct StorageLayout {
    helpers: HelperRegisters,
    symbol_bases: BTreeMap<SymbolId, u16>,
    temp_bases: BTreeMap<(SymbolId, usize), u16>,
}

/// Lowers typed IR into assembly, encoded words, and a final linker map.
pub fn compile_program(
    target: &TargetDevice,
    typed_program: &TypedProgram,
    ir_program: &IrProgram,
    diagnostics: &mut DiagnosticBag,
) -> Option<BackendOutput> {
    let layout = StorageAllocator::new(target.allocatable_gpr)
        .layout(typed_program, ir_program, diagnostics)?;

    let mut codegen = CodegenContext::new(target, typed_program, &layout);
    codegen.emit_program(ir_program, diagnostics);
    if diagnostics.has_errors() {
        return None;
    }

    let encoded = encode_program(&codegen.program, diagnostics)?;
    let map = build_map(typed_program, &layout, &encoded.labels);
    Some(BackendOutput {
        program: codegen.program,
        words: encoded.words,
        map,
    })
}

struct StorageAllocator<'a> {
    ranges: &'a [MemoryRange],
}

impl<'a> StorageAllocator<'a> {
    /// Creates a RAM allocator over the device's allocatable GPR ranges.
    fn new(ranges: &'a [MemoryRange]) -> Self {
        Self { ranges }
    }

    /// Assigns RAM slots for globals, locals, temps, and fixed ABI helper storage.
    fn layout(
        &self,
        typed_program: &TypedProgram,
        ir_program: &IrProgram,
        diagnostics: &mut DiagnosticBag,
    ) -> Option<StorageLayout> {
        let mut allocator = AddressAllocator::new(self.ranges);

        let Some(arg0_lo) = allocator.next_span(2) else {
            diagnostics.error("backend", None, "not enough RAM for ABI helper slots", None);
            return None;
        };
        let Some(arg1_lo) = allocator.next_span(2) else {
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
            arg0: RegisterPair {
                lo: arg0_lo,
                hi: arg0_lo + 1,
            },
            arg1: RegisterPair {
                lo: arg1_lo,
                hi: arg1_lo + 1,
            },
            return_high,
            scratch0,
            scratch1,
        };

        let mut symbol_bases = BTreeMap::new();
        for symbol in &typed_program.symbols {
            if let Some(addr) = symbol.fixed_address {
                symbol_bases.insert(symbol.id, addr);
                continue;
            }
            if matches!(symbol.kind, SymbolKind::Global | SymbolKind::Local | SymbolKind::Param) {
                let Some(base) = allocator.next_span(symbol.ty.byte_width()) else {
                    diagnostics.error(
                        "backend",
                        None,
                        format!("not enough allocatable RAM for symbol `{}`", symbol.name),
                        None,
                    );
                    return None;
                };
                symbol_bases.insert(symbol.id, base);
            }
        }

        let mut temp_bases = BTreeMap::new();
        for function in &ir_program.functions {
            for (temp, ty) in function.temp_types.iter().enumerate() {
                let Some(base) = allocator.next_span(ty.byte_width()) else {
                    diagnostics.error(
                        "backend",
                        None,
                        "not enough allocatable RAM for temporaries",
                        None,
                    );
                    return None;
                };
                temp_bases.insert((function.symbol, temp), base);
            }
        }

        Some(StorageLayout {
            helpers,
            symbol_bases,
            temp_bases,
        })
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
}

struct CodegenContext<'a> {
    target: &'a TargetDevice,
    typed_program: &'a TypedProgram,
    layout: &'a StorageLayout,
    program: AsmProgram,
    current_bank: u8,
    label_counter: usize,
}

impl<'a> CodegenContext<'a> {
    /// Creates a backend codegen context for one target, program, and storage layout.
    fn new(target: &'a TargetDevice, typed_program: &'a TypedProgram, layout: &'a StorageLayout) -> Self {
        Self {
            target,
            typed_program,
            layout,
            program: AsmProgram::new(),
            current_bank: 0,
            label_counter: 0,
        }
    }

    /// Emits vectors, startup, and all reachable function bodies for the IR program.
    fn emit_program(&mut self, ir_program: &IrProgram, diagnostics: &mut DiagnosticBag) {
        self.emit_vectors();
        self.emit_startup(ir_program, diagnostics);
        for function in &ir_program.functions {
            self.emit_function(function, diagnostics);
        }
    }

    /// Emits reset and interrupt vector stubs for the current target descriptor.
    fn emit_vectors(&mut self) {
        self.program.push(AsmLine::Org(self.target.vectors.reset));
        self.program.push(AsmLine::Instr(AsmInstr::SetPage("__start".to_string())));
        self.program.push(AsmLine::Instr(AsmInstr::Goto("__start".to_string())));
        self.program.push(AsmLine::Org(self.target.vectors.interrupt));
        self.program.push(AsmLine::Label("__interrupt_vector".to_string()));
        self.program.push(AsmLine::Instr(AsmInstr::Retfie));
        self.program.push(AsmLine::Org(self.target.vectors.interrupt + 1));
        self.program.push(AsmLine::Label("__start".to_string()));
    }

    /// Emits startup initialization for globals and transfers control to `main`.
    fn emit_startup(&mut self, ir_program: &IrProgram, diagnostics: &mut DiagnosticBag) {
        for global in &self.typed_program.globals {
            let symbol = &self.typed_program.symbols[global.symbol];
            let Some(base) = self.layout.symbol_bases.get(&global.symbol).copied() else {
                continue;
            };
            if let Some(initializer) = &global.initializer {
                let value = eval_const_expr(initializer);
                self.store_const_value(base, symbol.ty, value);
            } else {
                self.clear_slot(base, symbol.ty);
            }
        }

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
                "phase 2 requires `main` with no parameters",
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

    /// Emits one function body, including parameter moves into local slots.
    fn emit_function(&mut self, function: &IrFunction, diagnostics: &mut DiagnosticBag) {
        let name = self.symbol_name(function.symbol).to_string();
        self.program.push(AsmLine::Label(function_label(&name)));
        self.current_bank = 0;

        for (index, param) in function.params.iter().enumerate() {
            let param_symbol = &self.typed_program.symbols[*param];
            let Some(base) = self.layout.symbol_bases.get(param).copied() else {
                continue;
            };
            let slot = match index {
                0 => self.layout.helpers.arg0,
                1 => self.layout.helpers.arg1,
                _ => {
                    diagnostics.error(
                        "backend",
                        None,
                        "phase 2 ABI supports at most two parameters",
                        None,
                    );
                    continue;
                }
            };
            self.copy_register_pair_to_slot(slot, base, param_symbol.ty);
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
                let dst_base = self.temp_base(function.symbol, *dst);
                self.store_const_value(dst_base, dst_ty, i64::from(self.symbol_base(*symbol)));
            }
            IrInstr::Cast { dst, kind, src } => {
                let dst_ty = function.temp_types[*dst];
                let dst_base = self.temp_base(function.symbol, *dst);
                self.emit_cast(function.symbol, *src, *kind, dst_ty, dst_base);
            }
            IrInstr::Unary { dst, op, src } => {
                let dst_ty = function.temp_types[*dst];
                let dst_base = self.temp_base(function.symbol, *dst);
                match op {
                    UnaryOp::Negate => self.emit_negate(function.symbol, *src, dst_ty, dst_base),
                    UnaryOp::BitwiseNot => {
                        self.emit_per_byte_unary(function.symbol, *src, dst_ty, dst_base, |this, sym, operand, ty, byte| {
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
                        self.clear_slot(dst_base, dst_ty);
                    }
                }
            }
            IrInstr::Binary { dst, op, lhs, rhs } => {
                let dst_ty = function.temp_types[*dst];
                let dst_base = self.temp_base(function.symbol, *dst);
                match op {
                    BinaryOp::Add => self.emit_add(function.symbol, *lhs, *rhs, dst_ty, dst_base),
                    BinaryOp::Sub => self.emit_sub(function.symbol, *lhs, *rhs, dst_ty, dst_base),
                    BinaryOp::BitAnd => {
                        self.emit_per_byte_binary(
                            function.symbol,
                            *lhs,
                            *rhs,
                            dst_ty,
                            dst_base,
                            |_this, f| AsmInstr::Andwf { f, d: Dest::W },
                        );
                    }
                    BinaryOp::BitOr => {
                        self.emit_per_byte_binary(
                            function.symbol,
                            *lhs,
                            *rhs,
                            dst_ty,
                            dst_base,
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
                            dst_base,
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
                        self.clear_slot(dst_base, dst_ty);
                    }
                    BinaryOp::Multiply | BinaryOp::Divide | BinaryOp::Modulo => {
                        diagnostics.error(
                            "backend",
                            None,
                            format!("operation `{op:?}` is not implemented in phase 2"),
                            Some("use +, -, &, |, ^, ==, !=, <, <=, >, >= in phase 2".to_string()),
                        );
                        self.clear_slot(dst_base, dst_ty);
                    }
                }
            }
            IrInstr::Store { target, value } => {
                let target_ty = self.symbol_type(*target);
                let base = self.symbol_base(*target);
                self.copy_operand_to_slot(function.symbol, *value, target_ty, base);
            }
            IrInstr::LoadIndirect { dst, ptr } => {
                let dst_ty = function.temp_types[*dst];
                let dst_base = self.temp_base(function.symbol, *dst);
                self.emit_indirect_load(function.symbol, *ptr, dst_ty, dst_base);
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
                self.program.push(AsmLine::Instr(AsmInstr::Return));
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

    /// Lowers a direct call using the fixed helper-slot ABI and captures return values.
    fn emit_call(
        &mut self,
        function: &IrFunction,
        callee: SymbolId,
        args: &[Operand],
        dst: Option<usize>,
        diagnostics: &mut DiagnosticBag,
    ) {
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
            let slot = match index {
                0 => self.layout.helpers.arg0,
                1 => self.layout.helpers.arg1,
                _ => {
                    diagnostics.error(
                        "backend",
                        None,
                        "phase 2 ABI supports at most two call arguments",
                        None,
                    );
                    continue;
                }
            };
            self.copy_operand_to_register_pair(function.symbol, *arg, param_ty, slot);
        }

        let label = function_label(self.symbol_name(callee));
        self.program.push(AsmLine::Instr(AsmInstr::SetPage(label.clone())));
        self.program.push(AsmLine::Instr(AsmInstr::Call(label)));

        if let Some(dst) = dst {
            let dst_ty = function.temp_types[dst];
            let dst_base = self.temp_base(function.symbol, dst);
            self.store_w_to_addr(dst_base);
            if dst_ty.byte_width() == 2 {
                self.load_addr_to_w(self.layout.helpers.return_high);
                self.store_w_to_addr(dst_base + 1);
            }
        }
    }

    /// Places a return operand into the Phase 2 return convention locations.
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
    fn emit_add(&mut self, function_symbol: SymbolId, lhs: Operand, rhs: Operand, ty: Type, dst_base: u16) {
        self.load_operand_byte_to_w(function_symbol, lhs, ty, 0);
        self.store_w_to_addr(dst_base);
        self.load_operand_byte_to_w(function_symbol, rhs, ty, 0);
        self.select_bank(dst_base);
        self.program.push(AsmLine::Instr(AsmInstr::Addwf {
            f: low7(dst_base),
            d: Dest::W,
        }));
        self.store_w_to_addr(dst_base);

        if ty.byte_width() == 2 {
            self.load_operand_byte_to_w(function_symbol, lhs, ty, 1);
            self.program.push(AsmLine::Instr(AsmInstr::Btfsc {
                f: low7(STATUS_ADDR),
                b: STATUS_C_BIT,
            }));
            self.program.push(AsmLine::Instr(AsmInstr::Addlw(1)));
            self.store_w_to_addr(dst_base + 1);
            self.load_operand_byte_to_w(function_symbol, rhs, ty, 1);
            self.select_bank(dst_base + 1);
            self.program.push(AsmLine::Instr(AsmInstr::Addwf {
                f: low7(dst_base + 1),
                d: Dest::W,
            }));
            self.store_w_to_addr(dst_base + 1);
        }
    }

    /// Emits 8-bit or 16-bit subtraction with explicit borrow propagation.
    fn emit_sub(&mut self, function_symbol: SymbolId, lhs: Operand, rhs: Operand, ty: Type, dst_base: u16) {
        self.load_operand_byte_to_w(function_symbol, lhs, ty, 0);
        self.store_w_to_addr(dst_base);
        self.load_operand_byte_to_w(function_symbol, rhs, ty, 0);
        self.select_bank(dst_base);
        self.program.push(AsmLine::Instr(AsmInstr::Subwf {
            f: low7(dst_base),
            d: Dest::W,
        }));
        self.store_w_to_addr(dst_base);

        if ty.byte_width() == 2 {
            self.load_operand_byte_to_w(function_symbol, lhs, ty, 1);
            self.program.push(AsmLine::Instr(AsmInstr::Btfss {
                f: low7(STATUS_ADDR),
                b: STATUS_C_BIT,
            }));
            self.program.push(AsmLine::Instr(AsmInstr::Addlw(0xFF)));
            self.store_w_to_addr(dst_base + 1);
            self.load_operand_byte_to_w(function_symbol, rhs, ty, 1);
            self.select_bank(dst_base + 1);
            self.program.push(AsmLine::Instr(AsmInstr::Subwf {
                f: low7(dst_base + 1),
                d: Dest::W,
            }));
            self.store_w_to_addr(dst_base + 1);
        }
    }

    /// Emits two's-complement negation for the requested integer width.
    fn emit_negate(&mut self, function_symbol: SymbolId, src: Operand, ty: Type, dst_base: u16) {
        self.clear_addr(dst_base);
        self.load_operand_byte_to_w(function_symbol, src, ty, 0);
        self.select_bank(dst_base);
        self.program.push(AsmLine::Instr(AsmInstr::Subwf {
            f: low7(dst_base),
            d: Dest::F,
        }));

        if ty.byte_width() == 2 {
            self.clear_addr(dst_base + 1);
            self.load_operand_byte_to_w(function_symbol, src, ty, 1);
            self.program.push(AsmLine::Instr(AsmInstr::Btfss {
                f: low7(STATUS_ADDR),
                b: STATUS_C_BIT,
            }));
            self.program.push(AsmLine::Instr(AsmInstr::Addlw(1)));
            self.select_bank(dst_base + 1);
            self.program.push(AsmLine::Instr(AsmInstr::Subwf {
                f: low7(dst_base + 1),
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
        dst_base: u16,
        mut instr_for_addr: F,
    ) where
        F: FnMut(&mut Self, u8) -> AsmInstr,
    {
        for byte in 0..ty.byte_width() {
            let addr = dst_base + byte as u16;
            self.load_operand_byte_to_w(function_symbol, lhs, ty, byte);
            self.store_w_to_addr(addr);
            self.load_operand_byte_to_w(function_symbol, rhs, ty, byte);
            self.select_bank(addr);
            let instr = instr_for_addr(self, low7(addr));
            self.program.push(AsmLine::Instr(instr));
            self.store_w_to_addr(addr);
        }
    }

    /// Applies a byte-wise unary emission callback across all bytes of a value.
    fn emit_per_byte_unary<F>(
        &mut self,
        function_symbol: SymbolId,
        src: Operand,
        ty: Type,
        dst_base: u16,
        mut emit_for_byte: F,
    ) where
        F: FnMut(&mut Self, SymbolId, Operand, Type, usize),
    {
        for byte in 0..ty.byte_width() {
            emit_for_byte(self, function_symbol, src, ty, byte);
            self.store_w_to_addr(dst_base + byte as u16);
        }
    }

    /// Lowers an explicit cast between supported integer widths and signedness modes.
    fn emit_cast(
        &mut self,
        function_symbol: SymbolId,
        src: Operand,
        kind: CastKind,
        dst_ty: Type,
        dst_base: u16,
    ) {
        match kind {
            CastKind::Bitcast => self.copy_operand_to_slot(function_symbol, src, dst_ty, dst_base),
            CastKind::Truncate => {
                self.load_operand_byte_to_w(function_symbol, src, dst_ty, 0);
                self.store_w_to_addr(dst_base);
            }
            CastKind::ZeroExtend => {
                self.load_operand_byte_to_w(function_symbol, src, Type::new(ScalarType::U8), 0);
                self.store_w_to_addr(dst_base);
                if dst_ty.byte_width() == 2 {
                    self.clear_addr(dst_base + 1);
                }
            }
            CastKind::SignExtend => {
                match src {
                    Operand::Constant(value) => {
                        let low = low_byte(value, Type::new(ScalarType::U8));
                        self.store_const_value(dst_base, Type::new(ScalarType::U8), i64::from(low));
                        if dst_ty.byte_width() == 2 {
                            let high = if (low & 0x80) != 0 { 0xFF } else { 0x00 };
                            self.store_const_value(dst_base + 1, Type::new(ScalarType::U8), i64::from(high));
                        }
                    }
                    Operand::Symbol(symbol) => {
                        let source_base = self.symbol_base(symbol);
                        self.copy_addr_to_addr(source_base, dst_base);
                        if dst_ty.byte_width() == 2 {
                            let positive = self.unique_label("sext_pos");
                            let end = self.unique_label("sext_end");
                            self.select_bank(source_base);
                            self.program.push(AsmLine::Instr(AsmInstr::Btfss {
                                f: low7(source_base),
                                b: 7,
                            }));
                            self.branch_to_label(&positive);
                            self.store_const_value(dst_base + 1, Type::new(ScalarType::U8), 0xFF);
                            self.branch_to_label(&end);
                            self.program.push(AsmLine::Label(positive));
                            self.clear_addr(dst_base + 1);
                            self.program.push(AsmLine::Label(end));
                        }
                    }
                    Operand::Temp(temp) => {
                        let source_base = self.temp_base(function_symbol, temp);
                        self.copy_addr_to_addr(source_base, dst_base);
                        if dst_ty.byte_width() == 2 {
                            let positive = self.unique_label("sext_pos");
                            let end = self.unique_label("sext_end");
                            self.select_bank(source_base);
                            self.program.push(AsmLine::Instr(AsmInstr::Btfss {
                                f: low7(source_base),
                                b: 7,
                            }));
                            self.branch_to_label(&positive);
                            self.store_const_value(dst_base + 1, Type::new(ScalarType::U8), 0xFF);
                            self.branch_to_label(&end);
                            self.program.push(AsmLine::Label(positive));
                            self.clear_addr(dst_base + 1);
                            self.program.push(AsmLine::Label(end));
                        }
                    }
                }
            }
        }
    }

    /// Copies an operand into the RAM slot reserved for one temporary.
    fn copy_operand_to_temp(&mut self, function_symbol: SymbolId, src: Operand, ty: Type, temp: usize) {
        let base = self.temp_base(function_symbol, temp);
        self.copy_operand_to_slot(function_symbol, src, ty, base);
    }

    /// Copies an operand into any RAM slot, respecting 8-bit or 16-bit width.
    fn copy_operand_to_slot(&mut self, function_symbol: SymbolId, src: Operand, ty: Type, dst_base: u16) {
        for byte in 0..ty.byte_width() {
            self.load_operand_byte_to_w(function_symbol, src, ty, byte);
            self.store_w_to_addr(dst_base + byte as u16);
        }
    }

    /// Copies an operand into one ABI register pair used for call arguments.
    fn copy_operand_to_register_pair(
        &mut self,
        function_symbol: SymbolId,
        src: Operand,
        ty: Type,
        dst: RegisterPair,
    ) {
        self.load_operand_byte_to_w(function_symbol, src, ty, 0);
        self.store_w_to_addr(dst.lo);
        if ty.byte_width() == 2 {
            self.load_operand_byte_to_w(function_symbol, src, ty, 1);
            self.store_w_to_addr(dst.hi);
        } else {
            self.clear_addr(dst.hi);
        }
    }

    /// Copies bytes from an ABI register pair into a symbol or local slot.
    fn copy_register_pair_to_slot(&mut self, src: RegisterPair, dst_base: u16, ty: Type) {
        self.load_addr_to_w(src.lo);
        self.store_w_to_addr(dst_base);
        if ty.byte_width() == 2 {
            self.load_addr_to_w(src.hi);
            self.store_w_to_addr(dst_base + 1);
        }
    }

    /// Copies one byte from one RAM address to another through `W`.
    fn copy_addr_to_addr(&mut self, src: u16, dst: u16) {
        self.load_addr_to_w(src);
        self.store_w_to_addr(dst);
    }

    /// Loads one indirectly addressed scalar object through `FSR/INDF` into a temp slot.
    fn emit_indirect_load(
        &mut self,
        function_symbol: SymbolId,
        ptr: Operand,
        ty: Type,
        dst_base: u16,
    ) {
        for byte in 0..ty.byte_width() {
            self.prepare_indirect_pointer(function_symbol, ptr, byte as u8);
            self.load_indirect_to_w();
            self.store_w_to_addr(dst_base + byte as u16);
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
            Operand::Symbol(symbol) => self.load_addr_to_w(self.symbol_base(symbol) + byte_index as u16),
            Operand::Temp(temp) => self.load_addr_to_w(self.temp_base(function_symbol, temp) + byte_index as u16),
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
        if (bank & 0x01) == 0 {
            self.program.push(AsmLine::Instr(AsmInstr::Bcf { f: status, b: 5 }));
        } else {
            self.program.push(AsmLine::Instr(AsmInstr::Bsf { f: status, b: 5 }));
        }
        if (bank & 0x02) == 0 {
            self.program.push(AsmLine::Instr(AsmInstr::Bcf { f: status, b: 6 }));
        } else {
            self.program.push(AsmLine::Instr(AsmInstr::Bsf { f: status, b: 6 }));
        }
        self.current_bank = bank;
    }

    /// Returns the RAM base address assigned to a symbol.
    fn symbol_base(&self, symbol: SymbolId) -> u16 {
        self.layout.symbol_bases[&symbol]
    }

    /// Returns the RAM base address assigned to a function-local temp.
    fn temp_base(&self, function_symbol: SymbolId, temp: usize) -> u16 {
        self.layout.temp_bases[&(function_symbol, temp)]
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
            layout
                .symbol_bases
                .get(&symbol.id)
                .copied()
                .map(|addr| (symbol.name.clone(), addr))
        })
        .collect::<Vec<_>>();
    data_symbols.sort_by_key(|(_, addr)| *addr);

    MapFile {
        code_symbols,
        data_symbols,
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

/// Returns the low seven address bits used by direct-register PIC16 instructions.
const fn low7(addr: u16) -> u8 {
    (addr & 0x7F) as u8
}

#[cfg(test)]
mod tests {
    use super::compile_program;
    use crate::backend::pic16::devices::DeviceRegistry;
    use crate::diagnostics::{DiagnosticBag, WarningProfile};
    use crate::frontend::semantic::{Symbol, SymbolKind, TypedFunction, TypedGlobal, TypedProgram};
    use crate::frontend::types::{ScalarType, StorageClass, Type};
    use crate::ir::model::{IrBlock, IrFunction, IrInstr, IrProgram, IrTerminator, Operand};
    use crate::common::source::Span;

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

    /// Builds one typed symbol used by the backend unit test fixture.
    fn symbol(id: usize, name: &str, ty: Type, kind: SymbolKind) -> Symbol {
        Symbol {
            id,
            name: name.to_string(),
            ty,
            storage_class: StorageClass::Auto,
            kind,
            span: Span::new(0, 0),
            fixed_address: None,
            is_defined: true,
            is_referenced: true,
            parameter_types: Vec::new(),
        }
    }
}
