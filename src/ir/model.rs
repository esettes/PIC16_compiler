use std::collections::BTreeSet;
use std::fmt::Write;

use crate::frontend::ast::{BinaryOp, UnaryOp};
use crate::frontend::semantic::SymbolId;
use crate::frontend::types::Type;

pub type TempId = usize;
pub type BlockId = usize;

#[derive(Clone, Debug)]
pub struct IrProgram {
    pub globals: Vec<SymbolId>,
    pub functions: Vec<IrFunction>,
}

#[derive(Clone, Debug)]
pub struct IrFunction {
    pub symbol: SymbolId,
    pub params: Vec<SymbolId>,
    pub locals: Vec<SymbolId>,
    pub blocks: Vec<IrBlock>,
    pub entry: BlockId,
    pub temp_types: Vec<Type>,
    pub return_type: Type,
}

#[derive(Clone, Debug)]
pub struct IrBlock {
    pub id: BlockId,
    pub name: String,
    pub instructions: Vec<IrInstr>,
    pub terminator: IrTerminator,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Operand {
    Constant(i64),
    Symbol(SymbolId),
    Temp(TempId),
}

#[derive(Clone, Debug)]
pub enum IrInstr {
    Copy {
        dst: TempId,
        src: Operand,
    },
    Unary {
        dst: TempId,
        op: UnaryOp,
        src: Operand,
    },
    Binary {
        dst: TempId,
        op: BinaryOp,
        lhs: Operand,
        rhs: Operand,
    },
    Store {
        target: SymbolId,
        value: Operand,
    },
    Call {
        dst: Option<TempId>,
        function: SymbolId,
        args: Vec<Operand>,
    },
}

#[derive(Clone, Debug)]
pub enum IrTerminator {
    Return(Option<Operand>),
    Jump(BlockId),
    Branch {
        condition: IrCondition,
        then_block: BlockId,
        else_block: BlockId,
    },
    Unreachable,
}

#[derive(Clone, Debug)]
pub enum IrCondition {
    NonZero(Operand),
    Compare {
        op: CompareOp,
        lhs: Operand,
        rhs: Operand,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CompareOp {
    Equal,
    NotEqual,
    Less,
    LessEqual,
    Greater,
    GreaterEqual,
}

impl IrProgram {
    pub fn render(&self) -> String {
        let mut output = String::new();
        for function in &self.functions {
            let _ = writeln!(output, "fn #{} entry=b{}", function.symbol, function.entry);
            for block in &function.blocks {
                let _ = writeln!(output, "  b{} ({}):", block.id, block.name);
                for instruction in &block.instructions {
                    let _ = writeln!(output, "    {}", render_instr(instruction));
                }
                let _ = writeln!(output, "    {}", render_terminator(&block.terminator));
            }
        }
        output
    }
}

impl IrFunction {
    pub fn reachable_blocks(&self) -> BTreeSet<BlockId> {
        let mut visited = BTreeSet::new();
        let mut work = vec![self.entry];
        while let Some(block) = work.pop() {
            if !visited.insert(block) {
                continue;
            }
            match &self.blocks[block].terminator {
                IrTerminator::Jump(target) => work.push(*target),
                IrTerminator::Branch {
                    then_block,
                    else_block,
                    ..
                } => {
                    work.push(*then_block);
                    work.push(*else_block);
                }
                IrTerminator::Return(_) | IrTerminator::Unreachable => {}
            }
        }
        visited
    }
}

fn render_instr(instr: &IrInstr) -> String {
    match instr {
        IrInstr::Copy { dst, src } => format!("t{dst} = {}", render_operand(*src)),
        IrInstr::Unary { dst, op, src } => {
            format!("t{dst} = {op:?} {}", render_operand(*src))
        }
        IrInstr::Binary { dst, op, lhs, rhs } => format!(
            "t{dst} = {} {op:?} {}",
            render_operand(*lhs),
            render_operand(*rhs)
        ),
        IrInstr::Store { target, value } => format!("s{target} = {}", render_operand(*value)),
        IrInstr::Call {
            dst,
            function,
            args,
        } => {
            let rendered_args = args
                .iter()
                .map(|arg| render_operand(*arg))
                .collect::<Vec<_>>()
                .join(", ");
            if let Some(dst) = dst {
                format!("t{dst} = call f{function}({rendered_args})")
            } else {
                format!("call f{function}({rendered_args})")
            }
        }
    }
}

fn render_terminator(term: &IrTerminator) -> String {
    match term {
        IrTerminator::Return(value) => format!(
            "return {}",
            value
                .map(render_operand)
                .unwrap_or_else(|| "void".to_string())
        ),
        IrTerminator::Jump(block) => format!("jump b{block}"),
        IrTerminator::Branch {
            condition,
            then_block,
            else_block,
        } => format!(
            "branch {} ? b{} : b{}",
            render_condition(condition),
            then_block,
            else_block
        ),
        IrTerminator::Unreachable => "unreachable".to_string(),
    }
}

fn render_condition(condition: &IrCondition) -> String {
    match condition {
        IrCondition::NonZero(value) => format!("{} != 0", render_operand(*value)),
        IrCondition::Compare { op, lhs, rhs } => {
            format!("{} {op:?} {}", render_operand(*lhs), render_operand(*rhs))
        }
    }
}

pub fn render_operand(operand: Operand) -> String {
    match operand {
        Operand::Constant(value) => value.to_string(),
        Operand::Symbol(symbol) => format!("s{symbol}"),
        Operand::Temp(temp) => format!("t{temp}"),
    }
}
