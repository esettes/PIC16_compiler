use std::collections::BTreeMap;

use crate::backend::pic16::devices::{MemoryRange, TargetDevice};
use crate::diagnostics::DiagnosticBag;
use crate::frontend::ast::{BinaryOp, UnaryOp};
use crate::frontend::semantic::{SymbolId, SymbolKind, TypedExpr, TypedExprKind, TypedProgram};
use crate::ir::model::{CompareOp, IrCondition, IrFunction, IrInstr, IrProgram, IrTerminator, Operand};
use crate::linker::map::MapFile;

use super::asm::{AsmInstr, AsmLine, AsmProgram, Dest};
use super::encoder::encode_program;

const STATUS_ADDR: u16 = 0x03;
const STATUS_Z_BIT: u8 = 2;
#[derive(Debug)]
pub struct BackendOutput {
    pub program: AsmProgram,
    pub words: BTreeMap<u16, u16>,
    pub map: MapFile,
}

#[derive(Clone, Copy, Debug)]
struct HelperRegisters {
    arg0: u16,
    arg1: u16,
    scratch: u16,
}

#[derive(Debug)]
struct StorageLayout {
    helpers: HelperRegisters,
    symbol_addresses: BTreeMap<SymbolId, u16>,
    temp_addresses: BTreeMap<(SymbolId, usize), u16>,
}

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
    fn new(ranges: &'a [MemoryRange]) -> Self {
        Self { ranges }
    }

    fn layout(
        &self,
        typed_program: &TypedProgram,
        ir_program: &IrProgram,
        diagnostics: &mut DiagnosticBag,
    ) -> Option<StorageLayout> {
        let mut allocator = AddressAllocator::new(self.ranges);
        let arg0 = allocator.next()?;
        let arg1 = allocator.next()?;
        let scratch = allocator.next()?;
        let helpers = HelperRegisters {
            arg0,
            arg1,
            scratch,
        };

        let mut symbol_addresses = BTreeMap::new();
        for symbol in &typed_program.symbols {
            if let Some(addr) = symbol.fixed_address {
                symbol_addresses.insert(symbol.id, addr);
                continue;
            }
            if matches!(symbol.kind, SymbolKind::Global | SymbolKind::Local | SymbolKind::Param) {
                let Some(addr) = allocator.next() else {
                    diagnostics.error(
                        "backend",
                        None,
                        "not enough allocatable RAM for globals/locals",
                        None,
                    );
                    return None;
                };
                symbol_addresses.insert(symbol.id, addr);
            }
        }

        let mut temp_addresses = BTreeMap::new();
        for function in &ir_program.functions {
            for temp in 0..function.temp_types.len() {
                let Some(addr) = allocator.next() else {
                    diagnostics.error(
                        "backend",
                        None,
                        "not enough allocatable RAM for temporaries",
                        None,
                    );
                    return None;
                };
                temp_addresses.insert((function.symbol, temp), addr);
            }
        }

        Some(StorageLayout {
            helpers,
            symbol_addresses,
            temp_addresses,
        })
    }
}

struct AddressAllocator<'a> {
    ranges: &'a [MemoryRange],
    range_index: usize,
    next_addr: u16,
}

impl<'a> AddressAllocator<'a> {
    fn new(ranges: &'a [MemoryRange]) -> Self {
        Self {
            ranges,
            range_index: 0,
            next_addr: ranges.first().map_or(0, |range| range.start),
        }
    }

    fn next(&mut self) -> Option<u16> {
        while let Some(range) = self.ranges.get(self.range_index).copied() {
            if self.next_addr > range.end {
                self.range_index += 1;
                if let Some(next) = self.ranges.get(self.range_index) {
                    self.next_addr = next.start;
                }
                continue;
            }
            let addr = self.next_addr;
            self.next_addr += 1;
            return Some(addr);
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

    fn emit_program(&mut self, ir_program: &IrProgram, diagnostics: &mut DiagnosticBag) {
        self.emit_vectors();
        self.emit_startup(ir_program, diagnostics);
        for function in &ir_program.functions {
            self.emit_function(function, diagnostics);
        }
    }

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

    fn emit_startup(&mut self, ir_program: &IrProgram, diagnostics: &mut DiagnosticBag) {
        for global in &self.typed_program.globals {
            let Some(addr) = self.layout.symbol_addresses.get(&global.symbol).copied() else {
                continue;
            };
            if let Some(initializer) = &global.initializer {
                let value = eval_const_expr(initializer).unwrap_or(0) as u8;
                self.store_const(addr, value);
            } else {
                self.clear_addr(addr);
            }
        }

        let Some(main_symbol) = self
            .typed_program
            .symbols
            .iter()
            .find(|symbol| symbol.kind == SymbolKind::Function && symbol.name == "main")
            .map(|symbol| symbol.id)
        else {
            diagnostics.error(
                "backend",
                None,
                "entry function `main` not found",
                None,
            );
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
                "v0.1 requires `main` with no parameters",
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

    fn emit_function(&mut self, function: &IrFunction, diagnostics: &mut DiagnosticBag) {
        let name = self.symbol_name(function.symbol).to_string();
        let function_label = function_label(&name);
        self.program.push(AsmLine::Label(function_label.clone()));
        self.current_bank = 0;

        for (index, param) in function.params.iter().enumerate() {
            let Some(param_addr) = self.layout.symbol_addresses.get(param).copied() else {
                continue;
            };
            let arg_addr = match index {
                0 => self.layout.helpers.arg0,
                1 => self.layout.helpers.arg1,
                _ => {
                    diagnostics.error(
                        "backend",
                        None,
                        "v0.1 backend supports at most two parameters",
                        None,
                    );
                    continue;
                }
            };
            self.load_addr_to_w(arg_addr);
            self.store_w_to_addr(param_addr);
        }

        let reachable = function.reachable_blocks();
        for block in &function.blocks {
            if !reachable.contains(&block.id) {
                continue;
            }
            let label = block_label(&name, block.id);
            self.program.push(AsmLine::Label(label));
            for instr in &block.instructions {
                self.emit_instr(function, instr, diagnostics);
            }
            self.emit_terminator(function, &block.terminator, diagnostics);
        }
    }

    fn emit_instr(&mut self, function: &IrFunction, instr: &IrInstr, diagnostics: &mut DiagnosticBag) {
        match instr {
            IrInstr::Copy { dst, src } => {
                self.load_operand_to_w(function.symbol, *src);
                self.store_w_to_temp(function.symbol, *dst);
            }
            IrInstr::Unary { dst, op, src } => match op {
                UnaryOp::Negate => {
                    let dst_addr = self.temp_addr(function.symbol, *dst);
                    self.program.push(AsmLine::Comment("negate".to_string()));
                    self.program.push(AsmLine::Instr(AsmInstr::Clrw));
                    self.store_w_to_addr(dst_addr);
                    self.load_operand_to_w(function.symbol, *src);
                    self.select_bank(dst_addr);
                    self.program.push(AsmLine::Instr(AsmInstr::Subwf {
                        f: low7(dst_addr),
                        d: Dest::W,
                    }));
                    self.store_w_to_addr(dst_addr);
                }
                UnaryOp::BitwiseNot => {
                    self.load_operand_to_w(function.symbol, *src);
                    self.program.push(AsmLine::Instr(AsmInstr::Xorlw(0xFF)));
                    self.store_w_to_temp(function.symbol, *dst);
                }
                UnaryOp::LogicalNot => {
                    let dst_addr = self.temp_addr(function.symbol, *dst);
                    match *src {
                        Operand::Constant(value) => self.store_const(dst_addr, u8::from(value == 0)),
                        _ => {
                            let false_label = self.unique_label("not_false");
                            let end_label = self.unique_label("not_end");
                            self.load_operand_to_w(function.symbol, *src);
                            self.program.push(AsmLine::Instr(AsmInstr::Btfss {
                                f: low7(STATUS_ADDR),
                                b: STATUS_Z_BIT,
                            }));
                            self.branch_to_label(&false_label);
                            self.store_const(dst_addr, 1);
                            self.branch_to_label(&end_label);
                            self.program.push(AsmLine::Label(false_label));
                            self.clear_addr(dst_addr);
                            self.program.push(AsmLine::Label(end_label));
                        }
                    }
                }
            },
            IrInstr::Binary { dst, op, lhs, rhs } => {
                let dst_addr = self.temp_addr(function.symbol, *dst);
                match op {
                    BinaryOp::Add
                    | BinaryOp::Sub
                    | BinaryOp::BitAnd
                    | BinaryOp::BitOr
                    | BinaryOp::BitXor => {
                        self.load_operand_to_w(function.symbol, *lhs);
                        self.store_w_to_addr(dst_addr);
                        self.load_operand_to_w(function.symbol, *rhs);
                        self.select_bank(dst_addr);
                        let instr = match op {
                            BinaryOp::Add => AsmInstr::Addwf {
                                f: low7(dst_addr),
                                d: Dest::W,
                            },
                            BinaryOp::Sub => AsmInstr::Subwf {
                                f: low7(dst_addr),
                                d: Dest::W,
                            },
                            BinaryOp::BitAnd => AsmInstr::Andwf {
                                f: low7(dst_addr),
                                d: Dest::W,
                            },
                            BinaryOp::BitOr => AsmInstr::Iorwf {
                                f: low7(dst_addr),
                                d: Dest::W,
                            },
                            BinaryOp::BitXor => AsmInstr::Xorwf {
                                f: low7(dst_addr),
                                d: Dest::W,
                            },
                            _ => unreachable!("filtered above"),
                        };
                        self.program.push(AsmLine::Instr(instr));
                        self.store_w_to_addr(dst_addr);
                    }
                    BinaryOp::Equal | BinaryOp::NotEqual | BinaryOp::LogicalAnd | BinaryOp::LogicalOr => {
                        diagnostics.error(
                            "backend",
                            None,
                            format!("IR should lower `{op:?}` before backend"),
                            None,
                        );
                        self.clear_addr(dst_addr);
                    }
                    BinaryOp::Multiply
                    | BinaryOp::Divide
                    | BinaryOp::Modulo
                    | BinaryOp::Less
                    | BinaryOp::LessEqual
                    | BinaryOp::Greater
                    | BinaryOp::GreaterEqual => {
                        diagnostics.error(
                            "backend",
                            None,
                            format!("operation `{op:?}` not yet supported in v0.1 backend"),
                            Some("use +, -, &, |, ^, ==, != and non-zero conditions in v0.1".to_string()),
                        );
                        self.clear_addr(dst_addr);
                    }
                }
            }
            IrInstr::Store { target, value } => {
                self.load_operand_to_w(function.symbol, *value);
                let addr = self.symbol_addr(*target);
                self.store_w_to_addr(addr);
            }
            IrInstr::Call {
                dst,
                function: callee,
                args,
            } => {
                for (index, arg) in args.iter().enumerate() {
                    let arg_addr = match index {
                        0 => self.layout.helpers.arg0,
                        1 => self.layout.helpers.arg1,
                        _ => {
                            diagnostics.error(
                                "backend",
                                None,
                                "v0.1 backend supports at most two call arguments",
                                None,
                            );
                            continue;
                        }
                    };
                    self.load_operand_to_w(function.symbol, *arg);
                    self.store_w_to_addr(arg_addr);
                }
                let label = function_label(self.symbol_name(*callee));
                self.program.push(AsmLine::Instr(AsmInstr::SetPage(label.clone())));
                self.program.push(AsmLine::Instr(AsmInstr::Call(label)));
                if let Some(dst) = dst {
                    self.store_w_to_temp(function.symbol, *dst);
                }
            }
        }
    }

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
                    self.load_operand_to_w(function.symbol, *value);
                }
                self.program.push(AsmLine::Instr(AsmInstr::Return));
            }
            IrTerminator::Jump(target) => {
                self.branch_to_label(&block_label(fn_name, *target));
            }
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

    fn emit_branch(
        &mut self,
        function: &IrFunction,
        condition: &IrCondition,
        then_label: &str,
        else_label: &str,
        diagnostics: &mut DiagnosticBag,
    ) {
        match condition {
            IrCondition::NonZero(Operand::Constant(value)) => {
                if *value != 0 {
                    self.branch_to_label(then_label);
                } else {
                    self.branch_to_label(else_label);
                }
            }
            IrCondition::NonZero(value) => {
                self.load_operand_to_w(function.symbol, *value);
                self.branch_on_status_zero(false, then_label, else_label);
            }
            IrCondition::Compare {
                op: CompareOp::Equal,
                lhs,
                rhs,
            } => {
                self.compute_compare_flags(function.symbol, *lhs, *rhs);
                self.branch_on_status_zero(true, then_label, else_label);
            }
            IrCondition::Compare {
                op: CompareOp::NotEqual,
                lhs,
                rhs,
            } => {
                self.compute_compare_flags(function.symbol, *lhs, *rhs);
                self.branch_on_status_zero(false, then_label, else_label);
            }
            IrCondition::Compare { op, .. } => {
                diagnostics.error(
                    "backend",
                    None,
                    format!("comparison `{op:?}` not yet supported in v0.1 backend"),
                    Some("use `==`, `!=`, or non-zero tests in v0.1".to_string()),
                );
                self.branch_to_label(else_label);
            }
        }
    }

    fn compute_compare_flags(&mut self, function_symbol: SymbolId, lhs: Operand, rhs: Operand) {
        self.load_operand_to_w(function_symbol, lhs);
        self.store_w_to_addr(self.layout.helpers.scratch);
        self.load_operand_to_w(function_symbol, rhs);
        let scratch = self.layout.helpers.scratch;
        self.select_bank(scratch);
        self.program.push(AsmLine::Instr(AsmInstr::Subwf {
            f: low7(scratch),
            d: Dest::W,
        }));
    }

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

    fn load_operand_to_w(&mut self, function_symbol: SymbolId, operand: Operand) {
        match operand {
            Operand::Constant(value) => self.emit_const_to_w(value as u8),
            Operand::Symbol(symbol) => self.load_addr_to_w(self.symbol_addr(symbol)),
            Operand::Temp(temp) => self.load_addr_to_w(self.temp_addr(function_symbol, temp)),
        }
    }

    fn load_addr_to_w(&mut self, addr: u16) {
        self.select_bank(addr);
        self.program.push(AsmLine::Instr(AsmInstr::Movf {
            f: low7(addr),
            d: Dest::W,
        }));
    }

    fn emit_const_to_w(&mut self, value: u8) {
        if value == 0 {
            self.program.push(AsmLine::Instr(AsmInstr::Clrw));
        } else {
            self.program.push(AsmLine::Instr(AsmInstr::Movlw(value)));
        }
    }

    fn store_const(&mut self, addr: u16, value: u8) {
        if value == 0 {
            self.clear_addr(addr);
        } else {
            self.emit_const_to_w(value);
            self.store_w_to_addr(addr);
        }
    }

    fn store_w_to_temp(&mut self, function_symbol: SymbolId, temp: usize) {
        let addr = self.temp_addr(function_symbol, temp);
        self.store_w_to_addr(addr);
    }

    fn store_w_to_addr(&mut self, addr: u16) {
        self.select_bank(addr);
        self.program.push(AsmLine::Instr(AsmInstr::Movwf(low7(addr))));
    }

    fn clear_addr(&mut self, addr: u16) {
        self.select_bank(addr);
        self.program.push(AsmLine::Instr(AsmInstr::Clrf(low7(addr))));
    }

    fn branch_to_label(&mut self, label: &str) {
        self.program.push(AsmLine::Instr(AsmInstr::SetPage(label.to_string())));
        self.program.push(AsmLine::Instr(AsmInstr::Goto(label.to_string())));
    }

    fn select_bank(&mut self, addr: u16) {
        let bank = ((addr >> 7) & 0x03) as u8;
        if bank == self.current_bank {
            return;
        }
        let status = low7(STATUS_ADDR);
        if (bank & 0x01) == 0 {
            self.program.push(AsmLine::Instr(AsmInstr::Bcf {
                f: status,
                b: 5,
            }));
        } else {
            self.program.push(AsmLine::Instr(AsmInstr::Bsf {
                f: status,
                b: 5,
            }));
        }
        if (bank & 0x02) == 0 {
            self.program.push(AsmLine::Instr(AsmInstr::Bcf {
                f: status,
                b: 6,
            }));
        } else {
            self.program.push(AsmLine::Instr(AsmInstr::Bsf {
                f: status,
                b: 6,
            }));
        }
        self.current_bank = bank;
    }

    fn symbol_addr(&self, symbol: SymbolId) -> u16 {
        self.layout.symbol_addresses[&symbol]
    }

    fn temp_addr(&self, function_symbol: SymbolId, temp: usize) -> u16 {
        self.layout.temp_addresses[&(function_symbol, temp)]
    }

    fn symbol_name(&self, symbol: SymbolId) -> &str {
        &self.typed_program.symbols[symbol].name
    }

    fn unique_label(&mut self, prefix: &str) -> String {
        let label = format!("__{prefix}_{}", self.label_counter);
        self.label_counter += 1;
        label
    }
}

fn low7(addr: u16) -> u8 {
    (addr & 0x7F) as u8
}

fn function_label(name: &str) -> String {
    format!("fn_{name}")
}

fn block_label(function_name: &str, block: usize) -> String {
    format!("fn_{function_name}_b{block}")
}

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
        .filter_map(|symbol| layout.symbol_addresses.get(&symbol.id).copied().map(|addr| (symbol.name.clone(), addr)))
        .collect::<Vec<_>>();
    data_symbols.sort_by_key(|(_, addr)| *addr);

    MapFile {
        code_symbols,
        data_symbols,
    }
}

fn eval_const_expr(expr: &TypedExpr) -> Option<i64> {
    match &expr.kind {
        TypedExprKind::IntLiteral(value) => Some(*value),
        TypedExprKind::Unary { op, expr } => {
            let value = eval_const_expr(expr)?;
            Some(match op {
                UnaryOp::Negate => (-value) & 0xFF,
                UnaryOp::LogicalNot => i64::from(value == 0),
                UnaryOp::BitwiseNot => (!value) & 0xFF,
            })
        }
        TypedExprKind::Binary { op, lhs, rhs } => {
            let lhs = eval_const_expr(lhs)?;
            let rhs = eval_const_expr(rhs)?;
            Some(match op {
                BinaryOp::Add => (lhs + rhs) & 0xFF,
                BinaryOp::Sub => (lhs - rhs) & 0xFF,
                BinaryOp::BitAnd => lhs & rhs,
                BinaryOp::BitOr => lhs | rhs,
                BinaryOp::BitXor => lhs ^ rhs,
                BinaryOp::Equal => i64::from(lhs == rhs),
                BinaryOp::NotEqual => i64::from(lhs != rhs),
                BinaryOp::LogicalAnd => i64::from(lhs != 0 && rhs != 0),
                BinaryOp::LogicalOr => i64::from(lhs != 0 || rhs != 0),
                BinaryOp::Multiply => (lhs * rhs) & 0xFF,
                BinaryOp::Divide => {
                    if rhs == 0 {
                        0
                    } else {
                        lhs / rhs
                    }
                }
                BinaryOp::Modulo => {
                    if rhs == 0 {
                        0
                    } else {
                        lhs % rhs
                    }
                }
                BinaryOp::Less => i64::from(lhs < rhs),
                BinaryOp::LessEqual => i64::from(lhs <= rhs),
                BinaryOp::Greater => i64::from(lhs > rhs),
                BinaryOp::GreaterEqual => i64::from(lhs >= rhs),
            })
        }
        TypedExprKind::Assign { .. }
        | TypedExprKind::Call { .. }
        | TypedExprKind::Symbol(_) => None,
    }
}
