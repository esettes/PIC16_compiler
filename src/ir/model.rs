use std::collections::BTreeSet;
use std::fmt::Write;

use crate::frontend::ast::{BinaryOp, UnaryOp};
use crate::frontend::semantic::SymbolId;
use crate::frontend::types::{CastKind, Type};

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
    AddrOf {
        dst: TempId,
        symbol: SymbolId,
    },
    Cast {
        dst: TempId,
        kind: CastKind,
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
    LoadIndirect {
        dst: TempId,
        ptr: Operand,
    },
    StoreIndirect {
        ptr: Operand,
        value: Operand,
        ty: Type,
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
    NonZero {
        value: Operand,
        ty: Type,
    },
    Compare {
        op: BinaryOp,
        lhs: Operand,
        rhs: Operand,
        ty: Type,
    },
}

impl IrProgram {
    /// Renders the IR program into the textual dump used for debugging artifacts.
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
    /// Returns the CFG blocks reachable from the function entry block.
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

/// Formats one IR instruction for human-readable dumps.
fn render_instr(instr: &IrInstr) -> String {
    match instr {
        IrInstr::Copy { dst, src } => format!("t{dst} = {}", render_operand(*src)),
        IrInstr::AddrOf { dst, symbol } => format!("t{dst} = &s{symbol}"),
        IrInstr::Cast { dst, kind, src } => {
            format!("t{dst} = {kind:?} {}", render_operand(*src))
        }
        IrInstr::Unary { dst, op, src } => {
            format!("t{dst} = {op:?} {}", render_operand(*src))
        }
        IrInstr::Binary { dst, op, lhs, rhs } => format!(
            "t{dst} = {} {op:?} {}",
            render_operand(*lhs),
            render_operand(*rhs)
        ),
        IrInstr::Store { target, value } => format!("s{target} = {}", render_operand(*value)),
        IrInstr::LoadIndirect { dst, ptr } => {
            format!("t{dst} = *{}", render_operand(*ptr))
        }
        IrInstr::StoreIndirect { ptr, value, .. } => {
            format!("*{} = {}", render_operand(*ptr), render_operand(*value))
        }
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

/// Formats one IR terminator for human-readable dumps.
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

/// Formats one IR branch condition for human-readable dumps.
fn render_condition(condition: &IrCondition) -> String {
    match condition {
        IrCondition::NonZero { value, .. } => format!("{} != 0", render_operand(*value)),
        IrCondition::Compare { op, lhs, rhs, .. } => {
            format!("{} {op:?} {}", render_operand(*lhs), render_operand(*rhs))
        }
    }
}

/// Formats an IR operand using symbolic temp and symbol ids.
pub fn render_operand(operand: Operand) -> String {
    match operand {
        Operand::Constant(value) => value.to_string(),
        Operand::Symbol(symbol) => format!("s{symbol}"),
        Operand::Temp(temp) => format!("t{temp}"),
    }
}
