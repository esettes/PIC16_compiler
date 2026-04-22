use std::collections::{BTreeMap, BTreeSet};

use crate::frontend::ast::{BinaryOp, UnaryOp};

use super::model::{IrCondition, IrFunction, IrInstr, IrProgram, IrTerminator, Operand};

pub fn constant_fold(program: &mut IrProgram) {
    for function in &mut program.functions {
        let mut constants = BTreeMap::new();
        for block in &mut function.blocks {
            for instr in &mut block.instructions {
                match instr.clone() {
                    IrInstr::Copy { dst, src } => {
                        if let Operand::Constant(value) = src {
                            constants.insert(dst, value);
                        }
                    }
                    IrInstr::Unary { dst, op, src } => {
                        let src = resolve_operand(src, &constants);
                        *instr = IrInstr::Unary {
                            dst,
                            op,
                            src,
                        };
                        if let Operand::Constant(value) = src {
                            let result = eval_unary(op, value);
                            *instr = IrInstr::Copy {
                                dst,
                                src: Operand::Constant(result),
                            };
                            constants.insert(dst, result);
                        }
                    }
                    IrInstr::Binary { dst, op, lhs, rhs } => {
                        let lhs = resolve_operand(lhs, &constants);
                        let rhs = resolve_operand(rhs, &constants);
                        *instr = IrInstr::Binary {
                            dst,
                            op,
                            lhs,
                            rhs,
                        };
                        if let (Operand::Constant(lhs), Operand::Constant(rhs)) = (lhs, rhs) {
                            let result = eval_binary(op, lhs, rhs);
                            *instr = IrInstr::Copy {
                                dst,
                                src: Operand::Constant(result),
                            };
                            constants.insert(dst, result);
                        }
                    }
                    IrInstr::Store { target, value } => {
                        *instr = IrInstr::Store {
                            target,
                            value: resolve_operand(value, &constants),
                        };
                    }
                    IrInstr::Call { dst, .. } => {
                        if let Some(dst) = dst {
                            constants.remove(&dst);
                        }
                    }
                }
            }
            match &mut block.terminator {
                IrTerminator::Return(value) => {
                    if let Some(value) = value {
                        *value = resolve_operand(*value, &constants);
                    }
                }
                IrTerminator::Branch { condition, .. } => match condition {
                    IrCondition::NonZero(value) => *value = resolve_operand(*value, &constants),
                    IrCondition::Compare { lhs, rhs, .. } => {
                        *lhs = resolve_operand(*lhs, &constants);
                        *rhs = resolve_operand(*rhs, &constants);
                    }
                },
                IrTerminator::Jump(_) | IrTerminator::Unreachable => {}
            }
        }
    }
}

pub fn dead_code_elimination(program: &mut IrProgram) {
    for function in &mut program.functions {
        let used_temps = collect_used_temps(function);
        for block in &mut function.blocks {
            block.instructions.retain(|instr| match instr {
                IrInstr::Copy { dst, .. }
                | IrInstr::Unary { dst, .. }
                | IrInstr::Binary { dst, .. } => used_temps.contains(dst),
                IrInstr::Store { .. } | IrInstr::Call { .. } => true,
            });
        }
    }
}

fn resolve_operand(operand: Operand, constants: &BTreeMap<usize, i64>) -> Operand {
    if let Operand::Temp(temp) = operand
        && let Some(value) = constants.get(&temp).copied()
    {
        return Operand::Constant(value);
    }
    operand
}

fn collect_used_temps(function: &IrFunction) -> BTreeSet<usize> {
    let mut temps = BTreeSet::new();
    for block in &function.blocks {
        for instr in &block.instructions {
            match instr {
                IrInstr::Copy { src, .. } | IrInstr::Unary { src, .. } => collect_operand(*src, &mut temps),
                IrInstr::Binary { lhs, rhs, .. } => {
                    collect_operand(*lhs, &mut temps);
                    collect_operand(*rhs, &mut temps);
                }
                IrInstr::Store { value, .. } => collect_operand(*value, &mut temps),
                IrInstr::Call { args, .. } => {
                    for arg in args {
                        collect_operand(*arg, &mut temps);
                    }
                }
            }
        }
        match &block.terminator {
            IrTerminator::Return(value) => {
                if let Some(value) = value {
                    collect_operand(*value, &mut temps);
                }
            }
            IrTerminator::Branch { condition, .. } => match condition {
                IrCondition::NonZero(value) => collect_operand(*value, &mut temps),
                IrCondition::Compare { lhs, rhs, .. } => {
                    collect_operand(*lhs, &mut temps);
                    collect_operand(*rhs, &mut temps);
                }
            },
            IrTerminator::Jump(_) | IrTerminator::Unreachable => {}
        }
    }
    temps
}

fn collect_operand(operand: Operand, temps: &mut BTreeSet<usize>) {
    if let Operand::Temp(temp) = operand {
        temps.insert(temp);
    }
}

fn eval_unary(op: UnaryOp, value: i64) -> i64 {
    match op {
        UnaryOp::Negate => (-value) & 0xFF,
        UnaryOp::LogicalNot => i64::from(value == 0),
        UnaryOp::BitwiseNot => (!value) & 0xFF,
    }
}

fn eval_binary(op: BinaryOp, lhs: i64, rhs: i64) -> i64 {
    match op {
        BinaryOp::Add => (lhs + rhs) & 0xFF,
        BinaryOp::Sub => (lhs - rhs) & 0xFF,
        BinaryOp::Multiply => (lhs * rhs) & 0xFF,
        BinaryOp::Divide => {
            if rhs == 0 {
                0
            } else {
                (lhs / rhs) & 0xFF
            }
        }
        BinaryOp::Modulo => {
            if rhs == 0 {
                0
            } else {
                lhs % rhs
            }
        }
        BinaryOp::BitAnd => lhs & rhs,
        BinaryOp::BitOr => lhs | rhs,
        BinaryOp::BitXor => lhs ^ rhs,
        BinaryOp::LogicalAnd => i64::from(lhs != 0 && rhs != 0),
        BinaryOp::LogicalOr => i64::from(lhs != 0 || rhs != 0),
        BinaryOp::Equal => i64::from(lhs == rhs),
        BinaryOp::NotEqual => i64::from(lhs != rhs),
        BinaryOp::Less => i64::from(lhs < rhs),
        BinaryOp::LessEqual => i64::from(lhs <= rhs),
        BinaryOp::Greater => i64::from(lhs > rhs),
        BinaryOp::GreaterEqual => i64::from(lhs >= rhs),
    }
}
