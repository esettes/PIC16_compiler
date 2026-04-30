use crate::frontend::ast::{BinaryOp, UnaryOp};
use crate::frontend::semantic::{
    SymbolId, TypedExpr, TypedExprKind, TypedFunction, TypedProgram, TypedStmt,
};
use crate::frontend::types::{CastKind, ScalarType, Type};
use crate::diagnostics::DiagnosticBag;

use super::model::{
    BlockId, IrBlock, IrCondition, IrFunction, IrInstr, IrProgram, IrTerminator, Operand, TempId,
};

pub struct IrLowerer<'a> {
    _target: &'a crate::backend::pic16::devices::TargetDevice,
}

impl<'a> IrLowerer<'a> {
    /// Creates an IR lowerer bound to one target descriptor.
    pub fn new(target: &'a crate::backend::pic16::devices::TargetDevice) -> Self {
        Self { _target: target }
    }

    /// Lowers a fully typed program into CFG-based IR for optimization and backend codegen.
    pub fn lower(&self, program: &TypedProgram, diagnostics: &mut DiagnosticBag) -> IrProgram {
        let globals = program.globals.iter().map(|global| global.symbol).collect();
        let mut functions = Vec::new();
        for function in &program.functions {
            if let Some(body) = &function.body {
                let mut builder = FunctionBuilder::new(function.symbol, function, body.clone());
                builder.is_interrupt = program.symbols[function.symbol].is_interrupt;
                functions.push(builder.lower(diagnostics));
            }
        }
        IrProgram { globals, functions }
    }
}

struct FunctionBuilder {
    symbol: SymbolId,
    is_interrupt: bool,
    params: Vec<SymbolId>,
    locals: Vec<SymbolId>,
    return_type: Type,
    blocks: Vec<IrBlock>,
    current: BlockId,
    temp_types: Vec<Type>,
    loop_stack: Vec<(BlockId, BlockId)>,
    break_stack: Vec<BlockId>,
    body: TypedStmt,
}

#[derive(Clone, Copy, Debug)]
struct SwitchCaseLabel {
    value: i64,
    block: BlockId,
}

impl FunctionBuilder {
    /// Initializes a function-level builder around a typed function body.
    fn new(symbol: SymbolId, function: &TypedFunction, body: TypedStmt) -> Self {
        Self {
            symbol,
            is_interrupt: false,
            params: function.params.clone(),
            locals: function.locals.clone(),
            return_type: function.return_type,
            blocks: vec![IrBlock {
                id: 0,
                name: "entry".to_string(),
                instructions: Vec::new(),
                terminator: IrTerminator::Unreachable,
            }],
            current: 0,
            temp_types: Vec::new(),
            loop_stack: Vec::new(),
            break_stack: Vec::new(),
            body,
        }
    }

    /// Lowers the stored typed body and finalizes a complete IR function.
    fn lower(&mut self, _diagnostics: &mut DiagnosticBag) -> IrFunction {
        let body = self.body.clone();
        self.lower_stmt(&body);
        if matches!(self.blocks[self.current].terminator, IrTerminator::Unreachable) {
            self.blocks[self.current].terminator = if self.return_type.scalar == ScalarType::Void {
                IrTerminator::Return(None)
            } else {
                IrTerminator::Return(Some(Operand::Constant(0)))
            };
        }
        IrFunction {
            symbol: self.symbol,
            is_interrupt: self.is_interrupt,
            params: self.params.clone(),
            locals: self.locals.clone(),
            blocks: self.blocks.clone(),
            entry: 0,
            temp_types: self.temp_types.clone(),
            return_type: self.return_type,
        }
    }

    /// Lowers one typed statement into the current IR block graph.
    fn lower_stmt(&mut self, stmt: &TypedStmt) {
        match stmt {
            TypedStmt::Block(statements, _) => {
                for statement in statements {
                    self.lower_stmt(statement);
                }
            }
            TypedStmt::VarDecl(symbol, initializer, _) => {
                if let Some(initializer) = initializer {
                    let value = self.lower_expr(initializer);
                    self.emit(IrInstr::Store {
                        target: *symbol,
                        value,
                    });
                }
            }
            TypedStmt::Expr(expr, _) => {
                let _ = self.lower_expr(expr);
            }
            TypedStmt::Switch { expr, body, .. } => {
                let switch_value = self.lower_expr(expr);
                let end = self.new_block("switch.end");
                let body_start = self.new_block("switch.body");
                let mut cases = Vec::new();
                let mut label_blocks = std::collections::BTreeMap::new();
                let mut default_block = None;
                self.collect_switch_labels(body, &mut cases, &mut default_block, &mut label_blocks);

                self.lower_switch_dispatch(expr.ty, switch_value, &cases, default_block, end);

                self.break_stack.push(end);
                self.current = body_start;
                self.lower_switch_body_stmt(body, &label_blocks);
                self.ensure_jump(end);
                self.break_stack.pop();

                self.current = end;
            }
            TypedStmt::Case { .. } | TypedStmt::Default { .. } => {
                unreachable!("case/default labels lower only through switch-body lowering")
            }
            TypedStmt::If {
                condition,
                then_branch,
                else_branch,
                ..
            } => {
                let then_block = self.new_block("if.then");
                let else_block = self.new_block("if.else");
                let join_block = self.new_block("if.end");
                let else_target = if else_branch.is_some() { else_block } else { join_block };
                self.lower_condition(condition, then_block, else_target);

                self.current = then_block;
                self.lower_stmt(then_branch);
                self.ensure_jump(join_block);

                if let Some(else_branch) = else_branch {
                    self.current = else_block;
                    self.lower_stmt(else_branch);
                    self.ensure_jump(join_block);
                }

                self.current = join_block;
            }
            TypedStmt::While {
                condition, body, ..
            } => {
                let header = self.new_block("while.head");
                let body_block = self.new_block("while.body");
                let end = self.new_block("while.end");
                self.ensure_jump(header);

                self.current = header;
                self.loop_stack.push((header, end));
                self.break_stack.push(end);
                self.lower_condition(condition, body_block, end);

                self.current = body_block;
                self.lower_stmt(body);
                self.ensure_jump(header);
                self.break_stack.pop();
                self.loop_stack.pop();

                self.current = end;
            }
            TypedStmt::DoWhile {
                body,
                condition,
                ..
            } => {
                let body_block = self.new_block("do.body");
                let cond_block = self.new_block("do.cond");
                let end = self.new_block("do.end");
                self.ensure_jump(body_block);

                self.loop_stack.push((cond_block, end));
                self.break_stack.push(end);
                self.current = body_block;
                self.lower_stmt(body);
                self.ensure_jump(cond_block);

                self.current = cond_block;
                self.lower_condition(condition, body_block, end);
                self.break_stack.pop();
                self.loop_stack.pop();

                self.current = end;
            }
            TypedStmt::For {
                init,
                condition,
                step,
                body,
                ..
            } => {
                if let Some(init) = init {
                    self.lower_stmt(init);
                }
                let header = self.new_block("for.head");
                let body_block = self.new_block("for.body");
                let step_block = self.new_block("for.step");
                let end = self.new_block("for.end");
                self.ensure_jump(header);

                self.current = header;
                self.loop_stack.push((step_block, end));
                self.break_stack.push(end);
                if let Some(condition) = condition {
                    self.lower_condition(condition, body_block, end);
                } else {
                    self.blocks[self.current].terminator = IrTerminator::Jump(body_block);
                }

                self.current = body_block;
                self.lower_stmt(body);
                self.ensure_jump(step_block);

                self.current = step_block;
                if let Some(step) = step {
                    let _ = self.lower_expr(step);
                }
                self.ensure_jump(header);
                self.break_stack.pop();
                self.loop_stack.pop();

                self.current = end;
            }
            TypedStmt::Return(value, _) => {
                let value = value.as_ref().map(|expr| self.lower_expr(expr));
                self.blocks[self.current].terminator = IrTerminator::Return(value);
            }
            TypedStmt::Break(_) => {
                if let Some(break_block) = self.break_stack.last().copied() {
                    self.blocks[self.current].terminator = IrTerminator::Jump(break_block);
                    self.current = self.new_block("dead.break");
                }
            }
            TypedStmt::Continue(_) => {
                if let Some((continue_block, _)) = self.loop_stack.last().copied() {
                    self.blocks[self.current].terminator = IrTerminator::Jump(continue_block);
                    self.current = self.new_block("dead.continue");
                }
            }
            TypedStmt::Empty(_) => {}
        }
    }

    /// Collects valid case/default labels in source order before lowering the switch body.
    fn collect_switch_labels(
        &mut self,
        stmt: &TypedStmt,
        cases: &mut Vec<SwitchCaseLabel>,
        default_block: &mut Option<BlockId>,
        label_blocks: &mut std::collections::BTreeMap<usize, BlockId>,
    ) {
        match stmt {
            TypedStmt::Block(statements, _) => {
                for statement in statements {
                    self.collect_switch_labels(statement, cases, default_block, label_blocks);
                }
            }
            TypedStmt::Case { value, body, span } => {
                let block = self.new_block("switch.case");
                cases.push(SwitchCaseLabel {
                    value: *value,
                    block,
                });
                label_blocks.insert(span.start, block);
                self.collect_switch_labels(body, cases, default_block, label_blocks);
            }
            TypedStmt::Default { body, span } => {
                let block = self.new_block("switch.default");
                *default_block = Some(block);
                label_blocks.insert(span.start, block);
                self.collect_switch_labels(body, cases, default_block, label_blocks);
            }
            TypedStmt::Switch { .. }
            | TypedStmt::VarDecl(_, _, _)
            | TypedStmt::Expr(_, _)
            | TypedStmt::If { .. }
            | TypedStmt::While { .. }
            | TypedStmt::DoWhile { .. }
            | TypedStmt::For { .. }
            | TypedStmt::Return(_, _)
            | TypedStmt::Break(_)
            | TypedStmt::Continue(_)
            | TypedStmt::Empty(_) => {}
        }
    }

    /// Emits one switch dispatch chain using ordinary typed compare branches.
    fn lower_switch_dispatch(
        &mut self,
        switch_ty: Type,
        switch_value: Operand,
        cases: &[SwitchCaseLabel],
        default_block: Option<BlockId>,
        end_block: BlockId,
    ) {
        let default_target = default_block.unwrap_or(end_block);
        if cases.is_empty() {
            self.blocks[self.current].terminator = IrTerminator::Jump(default_target);
            return;
        }

        for (index, case) in cases.iter().enumerate() {
            let miss = if index + 1 == cases.len() {
                default_target
            } else {
                self.new_block("switch.next")
            };
            self.blocks[self.current].terminator = IrTerminator::Branch {
                condition: IrCondition::Compare {
                    op: BinaryOp::Equal,
                    lhs: switch_value,
                    rhs: Operand::Constant(case.value),
                    ty: switch_ty,
                },
                then_block: case.block,
                else_block: miss,
            };
            self.current = miss;
        }
    }

    /// Lowers one switch body while routing case/default labels to preallocated blocks.
    fn lower_switch_body_stmt(
        &mut self,
        stmt: &TypedStmt,
        label_blocks: &std::collections::BTreeMap<usize, BlockId>,
    ) {
        match stmt {
            TypedStmt::Block(statements, _) => {
                for statement in statements {
                    self.lower_switch_body_stmt(statement, label_blocks);
                }
            }
            TypedStmt::Case { body, span, .. } | TypedStmt::Default { body, span, .. } => {
                let block = *label_blocks
                    .get(&span.start)
                    .expect("switch label block collected");
                self.ensure_jump(block);
                self.current = block;
                self.lower_switch_body_stmt(body, label_blocks);
            }
            _ => self.lower_stmt(stmt),
        }
    }

    /// Lowers one typed expression into an IR operand, creating temps as needed.
    fn lower_expr(&mut self, expr: &TypedExpr) -> Operand {
        match &expr.kind {
            TypedExprKind::IntLiteral(value) => Operand::Constant(*value),
            TypedExprKind::Symbol(symbol) => {
                if expr.ty.is_array() {
                    self.lower_lvalue_address(expr)
                } else {
                    Operand::Symbol(*symbol)
                }
            }
            TypedExprKind::ArrayDecay(value) | TypedExprKind::AddressOf(value) => {
                self.lower_lvalue_address(value)
            }
            TypedExprKind::Cast { kind, expr: value } => {
                let src = self.lower_expr(value);
                match kind {
                    CastKind::Bitcast if value.ty.bit_width() == expr.ty.bit_width() => src,
                    _ => {
                        let dst = self.new_temp(expr.ty);
                        self.emit(IrInstr::Cast {
                            dst,
                            kind: *kind,
                            src,
                        });
                        Operand::Temp(dst)
                    }
                }
            }
            TypedExprKind::Unary { op, expr: operand } => {
                if *op == UnaryOp::LogicalNot {
                    return self.lower_boolean_to_value(expr);
                }
                let src = self.lower_expr(operand);
                let dst = self.new_temp(expr.ty);
                self.emit(IrInstr::Unary {
                    dst,
                    op: *op,
                    src,
                });
                Operand::Temp(dst)
            }
            TypedExprKind::Deref(pointer) => {
                let ptr = self.lower_expr(pointer);
                let dst = self.new_temp(expr.ty);
                self.emit(IrInstr::LoadIndirect { dst, ptr });
                Operand::Temp(dst)
            }
            TypedExprKind::Binary { op, lhs, rhs } => {
                if is_boolean_like(*op) {
                    return self.lower_boolean_to_value(expr);
                }
                let lhs = self.lower_expr(lhs);
                let rhs = self.lower_expr(rhs);
                let dst = self.new_temp(expr.ty);
                self.emit(IrInstr::Binary {
                    dst,
                    op: *op,
                    lhs,
                    rhs,
                });
                Operand::Temp(dst)
            }
            TypedExprKind::Assign { target, value } => {
                let value_operand = self.lower_expr(value);
                match &target.kind {
                    TypedExprKind::Symbol(symbol) => self.emit(IrInstr::Store {
                        target: *symbol,
                        value: value_operand,
                    }),
                    TypedExprKind::Deref(pointer) => {
                        let ptr = self.lower_expr(pointer);
                        self.emit(IrInstr::StoreIndirect {
                            ptr,
                            value: value_operand,
                            ty: target.ty,
                        });
                    }
                    _ => unreachable!("semantic only lowers assignable places here"),
                }
                value_operand
            }
            TypedExprKind::StructAssign { target, value, size } => {
                let dst_base = self.lower_lvalue_address(target);
                let src_base = self.lower_lvalue_address(value);
                let byte_ty = Type::new(ScalarType::U8);
                for offset in 0..*size {
                    let src_ptr = self.offset_pointer(src_base, offset);
                    let dst_ptr = self.offset_pointer(dst_base, offset);
                    let byte = self.new_temp(byte_ty);
                    self.emit(IrInstr::LoadIndirect {
                        dst: byte,
                        ptr: src_ptr,
                    });
                    self.emit(IrInstr::StoreIndirect {
                        ptr: dst_ptr,
                        value: Operand::Temp(byte),
                        ty: byte_ty,
                    });
                }
                Operand::Constant(0)
            }
            TypedExprKind::Call { function, args } => {
                let args = args.iter().map(|arg| self.lower_expr(arg)).collect::<Vec<_>>();
                if expr.ty.is_void() {
                    self.emit(IrInstr::Call {
                        dst: None,
                        function: *function,
                        args,
                    });
                    Operand::Constant(0)
                } else {
                    let dst = self.new_temp(expr.ty);
                    self.emit(IrInstr::Call {
                        dst: Some(dst),
                        function: *function,
                        args,
                    });
                    Operand::Temp(dst)
                }
            }
        }
    }

    /// Lowers one lvalue expression into a pointer-valued IR operand.
    fn lower_lvalue_address(&mut self, expr: &TypedExpr) -> Operand {
        match &expr.kind {
            TypedExprKind::Symbol(symbol) => {
                let ptr_ty = if expr.ty.is_array() {
                    expr.ty.decay()
                } else {
                    expr.ty.pointer_to()
                };
                let dst = self.new_temp(ptr_ty);
                self.emit(IrInstr::AddrOf {
                    dst,
                    symbol: *symbol,
                });
                Operand::Temp(dst)
            }
            TypedExprKind::Deref(pointer) => self.lower_expr(pointer),
            TypedExprKind::ArrayDecay(value) | TypedExprKind::AddressOf(value) => {
                self.lower_lvalue_address(value)
            }
            _ => unreachable!("semantic only forms addressable lvalues here"),
        }
    }

    /// Emits raw byte-pointer arithmetic used for aggregate copies.
    fn offset_pointer(&mut self, base: Operand, offset: usize) -> Operand {
        if offset == 0 {
            return base;
        }
        let byte_ptr_ty = Type::new(ScalarType::U8).pointer_to();
        let dst = self.new_temp(byte_ptr_ty);
        self.emit(IrInstr::Binary {
            dst,
            op: BinaryOp::Add,
            lhs: base,
            rhs: Operand::Constant(offset as i64),
        });
        Operand::Temp(dst)
    }

    /// Materializes a boolean-like expression as a normalized `0` or `1` temp.
    fn lower_boolean_to_value(&mut self, expr: &TypedExpr) -> Operand {
        let result = self.new_temp(Type::new(ScalarType::U8));
        let true_block = self.new_block("bool.true");
        let false_block = self.new_block("bool.false");
        let end_block = self.new_block("bool.end");
        self.lower_condition(expr, true_block, false_block);

        self.current = true_block;
        self.emit(IrInstr::Copy {
            dst: result,
            src: Operand::Constant(1),
        });
        self.ensure_jump(end_block);

        self.current = false_block;
        self.emit(IrInstr::Copy {
            dst: result,
            src: Operand::Constant(0),
        });
        self.ensure_jump(end_block);

        self.current = end_block;
        Operand::Temp(result)
    }

    /// Lowers a typed expression in branch position into an IR condition terminator.
    fn lower_condition(&mut self, expr: &TypedExpr, then_block: BlockId, else_block: BlockId) {
        match &expr.kind {
            TypedExprKind::Unary {
                op: UnaryOp::LogicalNot,
                expr,
            } => self.lower_condition(expr, else_block, then_block),
            TypedExprKind::Binary {
                op: BinaryOp::LogicalAnd,
                lhs,
                rhs,
            } => {
                let rhs_block = self.new_block("and.rhs");
                self.lower_condition(lhs, rhs_block, else_block);
                self.current = rhs_block;
                self.lower_condition(rhs, then_block, else_block);
            }
            TypedExprKind::Binary {
                op: BinaryOp::LogicalOr,
                lhs,
                rhs,
            } => {
                let rhs_block = self.new_block("or.rhs");
                self.lower_condition(lhs, then_block, rhs_block);
                self.current = rhs_block;
                self.lower_condition(rhs, then_block, else_block);
            }
            TypedExprKind::Binary { op, lhs, rhs } if is_comparison(*op) => {
                let compare_ty = lhs.ty;
                let lhs = self.lower_expr(lhs);
                let rhs = self.lower_expr(rhs);
                self.blocks[self.current].terminator = IrTerminator::Branch {
                    condition: IrCondition::Compare {
                        op: *op,
                        lhs,
                        rhs,
                        ty: compare_ty,
                    },
                    then_block,
                    else_block,
                };
            }
            _ => {
                let value = self.lower_expr(expr);
                self.blocks[self.current].terminator = IrTerminator::Branch {
                    condition: IrCondition::NonZero {
                        value,
                        ty: expr.ty,
                    },
                    then_block,
                    else_block,
                };
            }
        }
    }

    /// Appends one instruction to the current IR block.
    fn emit(&mut self, instr: IrInstr) {
        self.blocks[self.current].instructions.push(instr);
    }

    /// Allocates a new temp id and records its type for later passes and codegen.
    fn new_temp(&mut self, ty: Type) -> TempId {
        let id = self.temp_types.len();
        self.temp_types.push(ty);
        id
    }

    /// Creates a new CFG block and returns its block id.
    fn new_block(&mut self, name: &str) -> BlockId {
        let id = self.blocks.len();
        self.blocks.push(IrBlock {
            id,
            name: name.to_string(),
            instructions: Vec::new(),
            terminator: IrTerminator::Unreachable,
        });
        id
    }

    /// Inserts a jump only when the current block has no terminator yet.
    fn ensure_jump(&mut self, target: BlockId) {
        if matches!(self.blocks[self.current].terminator, IrTerminator::Unreachable) {
            self.blocks[self.current].terminator = IrTerminator::Jump(target);
        }
    }
}

/// Returns true when an operator lowers through boolean branch materialization.
fn is_boolean_like(op: BinaryOp) -> bool {
    matches!(
        op,
        BinaryOp::LogicalAnd
            | BinaryOp::LogicalOr
            | BinaryOp::Equal
            | BinaryOp::NotEqual
            | BinaryOp::Less
            | BinaryOp::LessEqual
            | BinaryOp::Greater
            | BinaryOp::GreaterEqual
    )
}

/// Returns true when an operator should lower into a typed compare condition.
fn is_comparison(op: BinaryOp) -> bool {
    matches!(
        op,
        BinaryOp::Equal
            | BinaryOp::NotEqual
            | BinaryOp::Less
            | BinaryOp::LessEqual
            | BinaryOp::Greater
            | BinaryOp::GreaterEqual
    )
}

#[cfg(test)]
mod tests {
    use super::IrLowerer;
    use crate::backend::pic16::devices::DeviceRegistry;
    use crate::common::source::Span;
    use crate::diagnostics::{DiagnosticBag, WarningProfile};
    use crate::frontend::ast::BinaryOp;
    use crate::frontend::semantic::{
        Symbol, SymbolKind, TypedExpr, TypedExprKind, TypedFunction, TypedProgram, TypedStmt,
        ValueCategory,
    };
    use crate::frontend::types::{ScalarType, StorageClass, Type};
    use crate::ir::model::{IrCondition, IrInstr, IrTerminator, Operand};

    #[test]
    /// Verifies relational expressions used as values lower through compare branches.
    fn lowers_relational_value_expr_through_compare_branch() {
        let registry = DeviceRegistry::new();
        let target = registry.device("pic16f628a").expect("device");
        let span = Span::new(0, 0);
        let u16_ty = Type::new(ScalarType::U16);
        let u8_ty = Type::new(ScalarType::U8);

        let program = TypedProgram {
            symbols: vec![
                symbol(0, "cmp", u8_ty, SymbolKind::Function),
                symbol(1, "lhs", u16_ty, SymbolKind::Param),
                symbol(2, "rhs", u16_ty, SymbolKind::Param),
            ],
            globals: Vec::new(),
            functions: vec![TypedFunction {
                symbol: 0,
                params: vec![1, 2],
                locals: Vec::new(),
                body: Some(TypedStmt::Return(
                    Some(TypedExpr {
                        kind: TypedExprKind::Binary {
                            op: BinaryOp::Less,
                            lhs: Box::new(TypedExpr {
                                kind: TypedExprKind::Symbol(1),
                                ty: u16_ty,
                                span,
                                value_category: ValueCategory::LValue,
                            }),
                            rhs: Box::new(TypedExpr {
                                kind: TypedExprKind::Symbol(2),
                                ty: u16_ty,
                                span,
                                value_category: ValueCategory::LValue,
                            }),
                        },
                        ty: u8_ty,
                        span,
                        value_category: ValueCategory::RValue,
                    }),
                    span,
                )),
                return_type: u8_ty,
                span,
            }],
        };

        let mut diagnostics = DiagnosticBag::new(WarningProfile::default());
        let ir = IrLowerer::new(target).lower(&program, &mut diagnostics);
        assert!(!diagnostics.has_errors());

        let function = &ir.functions[0];
        assert_eq!(function.temp_types, vec![u8_ty]);
        match &function.blocks[0].terminator {
            IrTerminator::Branch {
                condition:
                    IrCondition::Compare {
                        op: BinaryOp::Less,
                        lhs: Operand::Symbol(1),
                        rhs: Operand::Symbol(2),
                        ty,
                    },
                ..
            } => assert_eq!(*ty, u16_ty),
            other => panic!("expected compare branch, got {other:?}"),
        }
        let copies = function
            .blocks
            .iter()
            .flat_map(|block| block.instructions.iter())
            .filter_map(|instr| match instr {
                IrInstr::Copy {
                    dst: 0,
                    src: Operand::Constant(value),
                } => Some(*value),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert!(copies.contains(&0));
        assert!(copies.contains(&1));
    }

    /// Builds a minimal symbol used by the IR lowering regression test.
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
