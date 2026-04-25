use std::collections::{BTreeMap, BTreeSet};

use crate::common::integer::{eval_binary, eval_unary, normalize_value};
use crate::frontend::types::{CastKind, Type};

use super::model::{IrCondition, IrInstr, IrProgram, IrTerminator, Operand};

/// Replaces IR temp uses with constants and folds constant casts and arithmetic.
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
                    IrInstr::AddrOf { dst, symbol } => {
                        *instr = IrInstr::AddrOf { dst, symbol };
                        constants.remove(&dst);
                    }
                    IrInstr::Cast { dst, kind, src } => {
                        let src = resolve_operand(src, &constants);
                        *instr = IrInstr::Cast { dst, kind, src };
                        if let Operand::Constant(value) = src {
                            let result = apply_cast(kind, value, function.temp_types[dst]);
                            *instr = IrInstr::Copy {
                                dst,
                                src: Operand::Constant(result),
                            };
                            constants.insert(dst, result);
                        }
                    }
                    IrInstr::Unary { dst, op, src } => {
                        let src = resolve_operand(src, &constants);
                        *instr = IrInstr::Unary { dst, op, src };
                        if let Operand::Constant(value) = src {
                            let ty = function.temp_types[dst];
                            let result = eval_unary(op, value, ty, ty);
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
                        *instr = IrInstr::Binary { dst, op, lhs, rhs };
                        if let (Operand::Constant(lhs), Operand::Constant(rhs)) = (lhs, rhs) {
                            let ty = function.temp_types[dst];
                            let result = eval_binary(op, lhs, rhs, ty, ty);
                            *instr = IrInstr::Copy {
                                dst,
                                src: Operand::Constant(result),
                            };
                            constants.insert(dst, result);
                        }
                    }
                    IrInstr::LoadIndirect { dst, ptr } => {
                        *instr = IrInstr::LoadIndirect {
                            dst,
                            ptr: resolve_operand(ptr, &constants),
                        };
                        constants.remove(&dst);
                    }
                    IrInstr::Store { target, value } => {
                        *instr = IrInstr::Store {
                            target,
                            value: resolve_operand(value, &constants),
                        };
                    }
                    IrInstr::StoreIndirect { ptr, value, ty } => {
                        *instr = IrInstr::StoreIndirect {
                            ptr: resolve_operand(ptr, &constants),
                            value: resolve_operand(value, &constants),
                            ty,
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
                    IrCondition::NonZero { value, .. } => *value = resolve_operand(*value, &constants),
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

/// Removes temp-producing instructions whose results do not feed later side effects.
pub fn dead_code_elimination(program: &mut IrProgram) {
    for function in &mut program.functions {
        let mut live_temps = BTreeSet::new();
        for block in &mut function.blocks {
            match &block.terminator {
                IrTerminator::Return(value) => {
                    if let Some(value) = value {
                        collect_operand(*value, &mut live_temps);
                    }
                }
                IrTerminator::Branch { condition, .. } => match condition {
                    IrCondition::NonZero { value, .. } => collect_operand(*value, &mut live_temps),
                    IrCondition::Compare { lhs, rhs, .. } => {
                        collect_operand(*lhs, &mut live_temps);
                        collect_operand(*rhs, &mut live_temps);
                    }
                },
                IrTerminator::Jump(_) | IrTerminator::Unreachable => {}
            }
        }

        for block in function.blocks.iter_mut().rev() {
            let mut retained = Vec::new();
            for instr in block.instructions.iter().rev() {
                match instr {
                    IrInstr::Copy { dst, src }
                    | IrInstr::Cast { dst, src, .. }
                    | IrInstr::Unary { dst, src, .. } => {
                        if live_temps.remove(dst) {
                            collect_operand(*src, &mut live_temps);
                            retained.push(instr.clone());
                        }
                    }
                    IrInstr::AddrOf { dst, .. } => {
                        if live_temps.remove(dst) {
                            retained.push(instr.clone());
                        }
                    }
                    IrInstr::Binary { dst, lhs, rhs, .. } => {
                        if live_temps.remove(dst) {
                            collect_operand(*lhs, &mut live_temps);
                            collect_operand(*rhs, &mut live_temps);
                            retained.push(instr.clone());
                        }
                    }
                    IrInstr::LoadIndirect { dst, ptr } => {
                        if live_temps.remove(dst) {
                            collect_operand(*ptr, &mut live_temps);
                            retained.push(instr.clone());
                        }
                    }
                    IrInstr::Store { value, .. } => {
                        collect_operand(*value, &mut live_temps);
                        retained.push(instr.clone());
                    }
                    IrInstr::StoreIndirect { ptr, value, .. } => {
                        collect_operand(*ptr, &mut live_temps);
                        collect_operand(*value, &mut live_temps);
                        retained.push(instr.clone());
                    }
                    IrInstr::Call { dst, args, .. } => {
                        if let Some(dst) = dst {
                            live_temps.remove(dst);
                        }
                        for arg in args {
                            collect_operand(*arg, &mut live_temps);
                        }
                        retained.push(instr.clone());
                    }
                }
            }
            retained.reverse();
            block.instructions = retained;
        }
    }
}

/// Replaces a temp operand with its known constant value when available.
fn resolve_operand(operand: Operand, constants: &BTreeMap<usize, i64>) -> Operand {
    if let Operand::Temp(temp) = operand
        && let Some(value) = constants.get(&temp).copied()
    {
        return Operand::Constant(value);
    }
    operand
}

/// Adds a temp operand to the current liveness set.
fn collect_operand(operand: Operand, temps: &mut BTreeSet<usize>) {
    if let Operand::Temp(temp) = operand {
        temps.insert(temp);
    }
}

/// Evaluates an IR cast in the same way the runtime backend would truncate or extend it.
fn apply_cast(kind: CastKind, value: i64, target_ty: Type) -> i64 {
    match kind {
        CastKind::ZeroExtend => normalize_value(value & 0xFF, target_ty),
        CastKind::SignExtend => {
            let low = value & 0xFF;
            if (low & 0x80) != 0 {
                normalize_value(low | !0xFF, target_ty)
            } else {
                normalize_value(low, target_ty)
            }
        }
        CastKind::Truncate | CastKind::Bitcast => normalize_value(value, target_ty),
    }
}

#[cfg(test)]
mod tests {
    use super::{constant_fold, dead_code_elimination};
    use crate::frontend::ast::BinaryOp;
    use crate::frontend::semantic::SymbolId;
    use crate::frontend::types::{CastKind, ScalarType, Type};
    use crate::ir::model::{IrBlock, IrCondition, IrFunction, IrInstr, IrProgram, IrTerminator, Operand};

    #[test]
    /// Checks that constant folding propagates cast results into compare conditions.
    fn folds_phase_two_casts_and_conditions() {
        let mut program = IrProgram {
            globals: Vec::new(),
            functions: vec![IrFunction {
                symbol: 0,
                is_interrupt: false,
                params: Vec::new(),
                locals: Vec::new(),
                entry: 0,
                temp_types: vec![
                    Type::new(ScalarType::U16),
                    Type::new(ScalarType::I16),
                    Type::new(ScalarType::U16),
                ],
                return_type: Type::new(ScalarType::Void),
                blocks: vec![
                    IrBlock {
                        id: 0,
                        name: "entry".to_string(),
                        instructions: vec![
                            IrInstr::Cast {
                                dst: 0,
                                kind: CastKind::ZeroExtend,
                                src: Operand::Constant(0xFF),
                            },
                            IrInstr::Cast {
                                dst: 1,
                                kind: CastKind::SignExtend,
                                src: Operand::Constant(0xFF),
                            },
                            IrInstr::Copy {
                                dst: 2,
                                src: Operand::Constant(2),
                            },
                        ],
                        terminator: IrTerminator::Branch {
                            condition: IrCondition::Compare {
                                op: BinaryOp::GreaterEqual,
                                lhs: Operand::Temp(0),
                                rhs: Operand::Temp(2),
                                ty: Type::new(ScalarType::U16),
                            },
                            then_block: 1,
                            else_block: 2,
                        },
                    },
                    leaf_block(1),
                    leaf_block(2),
                ],
            }],
        };

        constant_fold(&mut program);
        let instructions = &program.functions[0].blocks[0].instructions;
        assert!(matches!(
            instructions[0],
            IrInstr::Copy {
                dst: 0,
                src: Operand::Constant(0x00FF)
            }
        ));
        assert!(matches!(
            instructions[1],
            IrInstr::Copy {
                dst: 1,
                src: Operand::Constant(0xFFFF)
            }
        ));
        match &program.functions[0].blocks[0].terminator {
            IrTerminator::Branch {
                condition:
                    IrCondition::Compare {
                        lhs: Operand::Constant(0x00FF),
                        rhs: Operand::Constant(2),
                        ..
                    },
                ..
            } => {}
            other => panic!("expected folded compare condition, got {other:?}"),
        }
    }

    #[test]
    /// Verifies backward liveness drops dead temp chains completely.
    fn drops_unused_phase_two_temps() {
        let mut program = IrProgram {
            globals: Vec::new(),
            functions: vec![IrFunction {
                symbol: 0 as SymbolId,
                is_interrupt: false,
                params: Vec::new(),
                locals: Vec::new(),
                entry: 0,
                temp_types: vec![Type::new(ScalarType::U16), Type::new(ScalarType::U8)],
                return_type: Type::new(ScalarType::Void),
                blocks: vec![IrBlock {
                    id: 0,
                    name: "entry".to_string(),
                    instructions: vec![
                        IrInstr::Copy {
                            dst: 0,
                            src: Operand::Constant(1),
                        },
                        IrInstr::Cast {
                            dst: 1,
                            kind: CastKind::Truncate,
                            src: Operand::Temp(0),
                        },
                    ],
                    terminator: IrTerminator::Return(None),
                }],
            }],
        };

        dead_code_elimination(&mut program);
        assert!(program.functions[0].blocks[0].instructions.is_empty());
    }

    /// Builds a minimal return block used by the local IR pass tests.
    fn leaf_block(id: usize) -> IrBlock {
        IrBlock {
            id,
            name: format!("b{id}"),
            instructions: Vec::new(),
            terminator: IrTerminator::Return(None),
        }
    }
}
