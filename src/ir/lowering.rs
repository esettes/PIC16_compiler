use crate::frontend::ast::{BinaryOp, UnaryOp};
use crate::frontend::semantic::{
    SymbolId, TypedExpr, TypedExprKind, TypedFunction, TypedProgram, TypedStmt,
};
use crate::frontend::types::{ScalarType, Type};
use crate::diagnostics::DiagnosticBag;

use super::model::{
    BlockId, CompareOp, IrBlock, IrCondition, IrFunction, IrInstr, IrProgram, IrTerminator,
    Operand, TempId,
};

pub struct IrLowerer<'a> {
    _target: &'a crate::backend::pic16::devices::TargetDevice,
}

impl<'a> IrLowerer<'a> {
    pub fn new(target: &'a crate::backend::pic16::devices::TargetDevice) -> Self {
        Self { _target: target }
    }

    pub fn lower(&self, program: &TypedProgram, diagnostics: &mut DiagnosticBag) -> IrProgram {
        let globals = program.globals.iter().map(|global| global.symbol).collect();
        let mut functions = Vec::new();
        for function in &program.functions {
            if let Some(body) = &function.body {
                let mut builder = FunctionBuilder::new(function.symbol, function, body.clone());
                functions.push(builder.lower(diagnostics));
            }
        }
        IrProgram { globals, functions }
    }
}

struct FunctionBuilder {
    symbol: SymbolId,
    params: Vec<SymbolId>,
    locals: Vec<SymbolId>,
    return_type: Type,
    blocks: Vec<IrBlock>,
    current: BlockId,
    temp_types: Vec<Type>,
    loop_stack: Vec<(BlockId, BlockId)>,
    body: TypedStmt,
}

impl FunctionBuilder {
    fn new(symbol: SymbolId, function: &TypedFunction, body: TypedStmt) -> Self {
        Self {
            symbol,
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
            body,
        }
    }

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
            params: self.params.clone(),
            locals: self.locals.clone(),
            blocks: self.blocks.clone(),
            entry: 0,
            temp_types: self.temp_types.clone(),
            return_type: self.return_type,
        }
    }

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
                self.lower_condition(condition, body_block, end);

                self.current = body_block;
                self.lower_stmt(body);
                self.ensure_jump(header);
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
                self.current = body_block;
                self.lower_stmt(body);
                self.ensure_jump(cond_block);

                self.current = cond_block;
                self.lower_condition(condition, body_block, end);
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
                self.loop_stack.pop();

                self.current = end;
            }
            TypedStmt::Return(value, _) => {
                let value = value.as_ref().map(|expr| self.lower_expr(expr));
                self.blocks[self.current].terminator = IrTerminator::Return(value);
            }
            TypedStmt::Break(_) => {
                if let Some((_, break_block)) = self.loop_stack.last().copied() {
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

    fn lower_expr(&mut self, expr: &TypedExpr) -> Operand {
        match &expr.kind {
            TypedExprKind::IntLiteral(value) => Operand::Constant(*value),
            TypedExprKind::Symbol(symbol) => Operand::Symbol(*symbol),
            TypedExprKind::Unary { op, expr } => {
                let src = self.lower_expr(expr);
                let dst = self.new_temp(expr.ty);
                self.emit(IrInstr::Unary {
                    dst,
                    op: *op,
                    src,
                });
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
                self.emit(IrInstr::Store {
                    target: *target,
                    value: value_operand,
                });
                value_operand
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
                let lhs = self.lower_expr(lhs);
                let rhs = self.lower_expr(rhs);
                self.blocks[self.current].terminator = IrTerminator::Branch {
                    condition: IrCondition::Compare {
                        op: compare_op(*op),
                        lhs,
                        rhs,
                    },
                    then_block,
                    else_block,
                };
            }
            _ => {
                let value = self.lower_expr(expr);
                self.blocks[self.current].terminator = IrTerminator::Branch {
                    condition: IrCondition::NonZero(value),
                    then_block,
                    else_block,
                };
            }
        }
    }

    fn emit(&mut self, instr: IrInstr) {
        self.blocks[self.current].instructions.push(instr);
    }

    fn new_temp(&mut self, ty: Type) -> TempId {
        let id = self.temp_types.len();
        self.temp_types.push(ty);
        id
    }

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

    fn ensure_jump(&mut self, target: BlockId) {
        if matches!(self.blocks[self.current].terminator, IrTerminator::Unreachable) {
            self.blocks[self.current].terminator = IrTerminator::Jump(target);
        }
    }
}

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

fn compare_op(op: BinaryOp) -> CompareOp {
    match op {
        BinaryOp::Equal => CompareOp::Equal,
        BinaryOp::NotEqual => CompareOp::NotEqual,
        BinaryOp::Less => CompareOp::Less,
        BinaryOp::LessEqual => CompareOp::LessEqual,
        BinaryOp::Greater => CompareOp::Greater,
        BinaryOp::GreaterEqual => CompareOp::GreaterEqual,
        _ => unreachable!("comparison op"),
    }
}
