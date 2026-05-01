use std::collections::{BTreeMap, BTreeSet};

use crate::common::integer::{eval_binary, eval_unary, normalize_value};
use crate::frontend::types::{CastKind, Type};

use super::model::{IrCondition, IrInstr, IrProgram, IrTerminator, Operand};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ConstantFoldStats {
    pub operands_propagated: usize,
    pub expressions_folded: usize,
    pub branches_simplified: usize,
    pub unreachable_blocks_pruned: usize,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct DeadCodeStats {
    pub instructions_removed: usize,
    pub unreachable_blocks_cleared: usize,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct TempCompactionStats {
    pub temp_slots_removed: usize,
}

/// Replaces IR temp uses with constants and folds constant casts and arithmetic.
pub fn constant_fold(program: &mut IrProgram) -> ConstantFoldStats {
    let mut stats = ConstantFoldStats::default();
    for function in &mut program.functions {
        let mut constants = BTreeMap::new();
        for block in &mut function.blocks {
            for instr in &mut block.instructions {
                match instr.clone() {
                    IrInstr::Copy { dst, src } => {
                        let src = resolve_operand(src, &constants);
                        if src != original_copy_src(instr) {
                            stats.operands_propagated += 1;
                        }
                        *instr = IrInstr::Copy { dst, src };
                        if let Operand::Constant(value) = src {
                            constants.insert(dst, value);
                        } else {
                            constants.remove(&dst);
                        }
                    }
                    IrInstr::AddrOf { dst, symbol } => {
                        *instr = IrInstr::AddrOf { dst, symbol };
                        constants.remove(&dst);
                    }
                    IrInstr::Cast { dst, kind, src } => {
                        let src = resolve_operand(src, &constants);
                        if src != original_unary_src(instr) {
                            stats.operands_propagated += 1;
                        }
                        *instr = IrInstr::Cast { dst, kind, src };
                        if let Operand::Constant(value) = src {
                            let result = apply_cast(kind, value, function.temp_types[dst]);
                            *instr = IrInstr::Copy {
                                dst,
                                src: Operand::Constant(result),
                            };
                            constants.insert(dst, result);
                            stats.expressions_folded += 1;
                        }
                    }
                    IrInstr::Unary { dst, op, src } => {
                        let src = resolve_operand(src, &constants);
                        if src != original_unary_src(instr) {
                            stats.operands_propagated += 1;
                        }
                        *instr = IrInstr::Unary { dst, op, src };
                        if let Operand::Constant(value) = src {
                            let ty = function.temp_types[dst];
                            let result = eval_unary(op, value, ty, ty);
                            *instr = IrInstr::Copy {
                                dst,
                                src: Operand::Constant(result),
                            };
                            constants.insert(dst, result);
                            stats.expressions_folded += 1;
                        }
                    }
                    IrInstr::Binary { dst, op, lhs, rhs } => {
                        let original = (lhs, rhs);
                        let lhs = resolve_operand(lhs, &constants);
                        let rhs = resolve_operand(rhs, &constants);
                        if (lhs, rhs) != original {
                            stats.operands_propagated += usize::from(lhs != original.0)
                                + usize::from(rhs != original.1);
                        }
                        *instr = IrInstr::Binary { dst, op, lhs, rhs };
                        if let (Operand::Constant(lhs), Operand::Constant(rhs)) = (lhs, rhs) {
                            let ty = function.temp_types[dst];
                            let result = eval_binary(op, lhs, rhs, ty, ty);
                            *instr = IrInstr::Copy {
                                dst,
                                src: Operand::Constant(result),
                            };
                            constants.insert(dst, result);
                            stats.expressions_folded += 1;
                        }
                    }
                    IrInstr::LoadIndirect { dst, ptr } => {
                        let resolved = resolve_operand(ptr, &constants);
                        if resolved != ptr {
                            stats.operands_propagated += 1;
                        }
                        *instr = IrInstr::LoadIndirect {
                            dst,
                            ptr: resolved,
                        };
                        constants.remove(&dst);
                    }
                    IrInstr::Store { target, value } => {
                        let resolved = resolve_operand(value, &constants);
                        if resolved != value {
                            stats.operands_propagated += 1;
                        }
                        *instr = IrInstr::Store {
                            target,
                            value: resolved,
                        };
                    }
                    IrInstr::StoreIndirect { ptr, value, ty } => {
                        let resolved_ptr = resolve_operand(ptr, &constants);
                        let resolved_value = resolve_operand(value, &constants);
                        stats.operands_propagated += usize::from(resolved_ptr != ptr)
                            + usize::from(resolved_value != value);
                        *instr = IrInstr::StoreIndirect {
                            ptr: resolved_ptr,
                            value: resolved_value,
                            ty,
                        };
                    }
                    IrInstr::RomRead8 { dst, symbol, index } => {
                        let resolved = resolve_operand(index, &constants);
                        if resolved != index {
                            stats.operands_propagated += 1;
                        }
                        *instr = IrInstr::RomRead8 {
                            dst,
                            symbol,
                            index: resolved,
                        };
                        constants.remove(&dst);
                    }
                    IrInstr::Call {
                        dst,
                        function,
                        args,
                    } => {
                        let resolved_args = args
                            .iter()
                            .map(|arg| resolve_operand(*arg, &constants))
                            .collect::<Vec<_>>();
                        stats.operands_propagated += args
                            .iter()
                            .zip(&resolved_args)
                            .filter(|(before, after)| **before != **after)
                            .count();
                        *instr = IrInstr::Call {
                            dst,
                            function,
                            args: resolved_args,
                        };
                        if let Some(dst) = dst {
                            constants.remove(&dst);
                        }
                    }
                }
            }

            match &mut block.terminator {
                IrTerminator::Return(value) => {
                    if let Some(value) = value {
                        let resolved = resolve_operand(*value, &constants);
                        if resolved != *value {
                            stats.operands_propagated += 1;
                        }
                        *value = resolved;
                    }
                }
                IrTerminator::Branch { condition, .. } => match condition {
                    IrCondition::NonZero { value, .. } => {
                        let resolved = resolve_operand(*value, &constants);
                        if resolved != *value {
                            stats.operands_propagated += 1;
                        }
                        *value = resolved;
                    }
                    IrCondition::Compare { lhs, rhs, .. } => {
                        let resolved_lhs = resolve_operand(*lhs, &constants);
                        let resolved_rhs = resolve_operand(*rhs, &constants);
                        stats.operands_propagated += usize::from(resolved_lhs != *lhs)
                            + usize::from(resolved_rhs != *rhs);
                        *lhs = resolved_lhs;
                        *rhs = resolved_rhs;
                    }
                },
                IrTerminator::Jump(_) | IrTerminator::Unreachable => {}
            }
        }

        for block in &mut function.blocks {
            if let IrTerminator::Branch {
                condition,
                then_block,
                else_block,
            } = &block.terminator
                && let Some(value) = eval_condition(condition)
            {
                block.terminator = IrTerminator::Jump(if value { *then_block } else { *else_block });
                stats.branches_simplified += 1;
            }
        }

        let reachable = function.reachable_blocks();
        stats.unreachable_blocks_pruned += function.blocks.len().saturating_sub(reachable.len());
    }
    stats
}

/// Removes temp-producing instructions whose results do not feed later side effects.
pub fn dead_code_elimination(program: &mut IrProgram) -> DeadCodeStats {
    let mut stats = DeadCodeStats::default();
    for function in &mut program.functions {
        let reachable = function.reachable_blocks();
        let mut live_temps = BTreeSet::new();
        for block in &function.blocks {
            if !reachable.contains(&block.id) {
                continue;
            }
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
            if !reachable.contains(&block.id) {
                stats.unreachable_blocks_cleared += usize::from(!block.instructions.is_empty());
                stats.instructions_removed += block.instructions.len();
                block.instructions.clear();
                block.terminator = IrTerminator::Unreachable;
                continue;
            }
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
                    IrInstr::RomRead8 { dst, index, .. } => {
                        if live_temps.remove(dst) {
                            collect_operand(*index, &mut live_temps);
                            retained.push(instr.clone());
                        }
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
            stats.instructions_removed += block.instructions.len().saturating_sub(retained.len());
            retained.reverse();
            block.instructions = retained;
        }
    }
    stats
}

/// Removes unused temp slots and remaps surviving temps to a compact index space.
pub fn compact_temps(program: &mut IrProgram) -> TempCompactionStats {
    let mut stats = TempCompactionStats::default();
    for function in &mut program.functions {
        let reachable = function.reachable_blocks();
        let mut used = BTreeSet::new();
        for block in &function.blocks {
            if !reachable.contains(&block.id) {
                continue;
            }
            for instr in &block.instructions {
                collect_instr_temps(instr, &mut used);
            }
            collect_terminator_temps(&block.terminator, &mut used);
        }

        if used.len() == function.temp_types.len() {
            continue;
        }

        let old_types = function.temp_types.clone();
        let remap = used
            .iter()
            .enumerate()
            .map(|(new_id, old_id)| (*old_id, new_id))
            .collect::<BTreeMap<_, _>>();
        function.temp_types = used.iter().map(|id| old_types[*id]).collect();
        stats.temp_slots_removed += old_types.len().saturating_sub(function.temp_types.len());

        for block in &mut function.blocks {
            remap_block(block, &remap);
        }
    }
    stats
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

/// Collects all temp ids referenced or defined by one instruction.
fn collect_instr_temps(instr: &IrInstr, temps: &mut BTreeSet<usize>) {
    match instr {
        IrInstr::Copy { dst, src } | IrInstr::Cast { dst, src, .. } | IrInstr::Unary { dst, src, .. } => {
            temps.insert(*dst);
            collect_operand(*src, temps);
        }
        IrInstr::AddrOf { dst, .. } => {
            temps.insert(*dst);
        }
        IrInstr::Binary { dst, lhs, rhs, .. } => {
            temps.insert(*dst);
            collect_operand(*lhs, temps);
            collect_operand(*rhs, temps);
        }
        IrInstr::LoadIndirect { dst, ptr } => {
            temps.insert(*dst);
            collect_operand(*ptr, temps);
        }
        IrInstr::Store { value, .. } => collect_operand(*value, temps),
        IrInstr::StoreIndirect { ptr, value, .. } => {
            collect_operand(*ptr, temps);
            collect_operand(*value, temps);
        }
        IrInstr::RomRead8 { dst, index, .. } => {
            temps.insert(*dst);
            collect_operand(*index, temps);
        }
        IrInstr::Call { dst, args, .. } => {
            if let Some(dst) = dst {
                temps.insert(*dst);
            }
            for arg in args {
                collect_operand(*arg, temps);
            }
        }
    }
}

/// Collects temp ids referenced by one terminator.
fn collect_terminator_temps(terminator: &IrTerminator, temps: &mut BTreeSet<usize>) {
    match terminator {
        IrTerminator::Return(value) => {
            if let Some(value) = value {
                collect_operand(*value, temps);
            }
        }
        IrTerminator::Branch { condition, .. } => match condition {
            IrCondition::NonZero { value, .. } => collect_operand(*value, temps),
            IrCondition::Compare { lhs, rhs, .. } => {
                collect_operand(*lhs, temps);
                collect_operand(*rhs, temps);
            }
        },
        IrTerminator::Jump(_) | IrTerminator::Unreachable => {}
    }
}

/// Rewrites temp ids inside one block according to the compacted remapping table.
fn remap_block(block: &mut super::model::IrBlock, remap: &BTreeMap<usize, usize>) {
    for instr in &mut block.instructions {
        match instr {
            IrInstr::Copy { dst, src } | IrInstr::Cast { dst, src, .. } | IrInstr::Unary { dst, src, .. } => {
                *dst = remap[dst];
                *src = remap_operand(*src, remap);
            }
            IrInstr::AddrOf { dst, .. } => *dst = remap[dst],
            IrInstr::Binary { dst, lhs, rhs, .. } => {
                *dst = remap[dst];
                *lhs = remap_operand(*lhs, remap);
                *rhs = remap_operand(*rhs, remap);
            }
            IrInstr::LoadIndirect { dst, ptr } => {
                *dst = remap[dst];
                *ptr = remap_operand(*ptr, remap);
            }
            IrInstr::Store { value, .. } => *value = remap_operand(*value, remap),
            IrInstr::StoreIndirect { ptr, value, .. } => {
                *ptr = remap_operand(*ptr, remap);
                *value = remap_operand(*value, remap);
            }
            IrInstr::RomRead8 { dst, index, .. } => {
                *dst = remap[dst];
                *index = remap_operand(*index, remap);
            }
            IrInstr::Call { dst, args, .. } => {
                if let Some(dst) = dst {
                    *dst = remap[dst];
                }
                for arg in args {
                    *arg = remap_operand(*arg, remap);
                }
            }
        }
    }

    match &mut block.terminator {
        IrTerminator::Return(value) => {
            if let Some(value) = value {
                *value = remap_operand(*value, remap);
            }
        }
        IrTerminator::Branch { condition, .. } => match condition {
            IrCondition::NonZero { value, .. } => *value = remap_operand(*value, remap),
            IrCondition::Compare { lhs, rhs, .. } => {
                *lhs = remap_operand(*lhs, remap);
                *rhs = remap_operand(*rhs, remap);
            }
        },
        IrTerminator::Jump(_) | IrTerminator::Unreachable => {}
    }
}

/// Remaps one operand if it references a temp id.
fn remap_operand(operand: Operand, remap: &BTreeMap<usize, usize>) -> Operand {
    match operand {
        Operand::Temp(temp) => Operand::Temp(remap[&temp]),
        _ => operand,
    }
}

/// Evaluates a branch condition when it has folded to constants completely.
fn eval_condition(condition: &IrCondition) -> Option<bool> {
    match condition {
        IrCondition::NonZero { value, ty } => {
            let Operand::Constant(value) = value else {
                return None;
            };
            Some(normalize_value(*value, *ty) != 0)
        }
        IrCondition::Compare { op, lhs, rhs, ty } => {
            let (Operand::Constant(lhs), Operand::Constant(rhs)) = (lhs, rhs) else {
                return None;
            };
            Some(eval_binary(*op, *lhs, *rhs, *ty, Type::new(crate::frontend::types::ScalarType::U8)) != 0)
        }
    }
}

/// Returns the original `Copy` source without cloning the full instruction outside the caller.
fn original_copy_src(instr: &IrInstr) -> Operand {
    match instr {
        IrInstr::Copy { src, .. } => *src,
        _ => unreachable!("copy instruction"),
    }
}

/// Returns the original unary-like source operand.
fn original_unary_src(instr: &IrInstr) -> Operand {
    match instr {
        IrInstr::Cast { src, .. } | IrInstr::Unary { src, .. } => *src,
        _ => unreachable!("unary-like instruction"),
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
mod phase7_tests {
    use crate::frontend::ast::BinaryOp;
    use crate::frontend::types::{ScalarType, Type};
    use crate::ir::model::{
        IrBlock, IrCondition, IrFunction, IrInstr, IrProgram, IrTerminator, Operand,
    };

    use super::{compact_temps, constant_fold, dead_code_elimination};

    fn u8_ty() -> Type {
        Type::new(ScalarType::U8)
    }

    fn test_program(blocks: Vec<IrBlock>, temp_types: Vec<Type>) -> IrProgram {
        IrProgram {
            globals: Vec::new(),
            functions: vec![IrFunction {
                symbol: 0,
                is_interrupt: false,
                params: Vec::new(),
                locals: Vec::new(),
                blocks,
                entry: 0,
                temp_types,
                return_type: u8_ty(),
            }],
        }
    }

    #[test]
    fn constant_fold_simplifies_branch_to_jump() {
        let mut program = test_program(
            vec![
                IrBlock {
                    id: 0,
                    name: "entry".to_string(),
                    instructions: vec![IrInstr::Copy {
                        dst: 0,
                        src: Operand::Constant(1),
                    }],
                    terminator: IrTerminator::Branch {
                        condition: IrCondition::NonZero {
                            value: Operand::Temp(0),
                            ty: u8_ty(),
                        },
                        then_block: 1,
                        else_block: 2,
                    },
                },
                IrBlock {
                    id: 1,
                    name: "then".to_string(),
                    instructions: Vec::new(),
                    terminator: IrTerminator::Return(Some(Operand::Constant(7))),
                },
                IrBlock {
                    id: 2,
                    name: "else".to_string(),
                    instructions: Vec::new(),
                    terminator: IrTerminator::Return(Some(Operand::Constant(9))),
                },
            ],
            vec![u8_ty()],
        );

        let stats = constant_fold(&mut program);

        assert_eq!(stats.branches_simplified, 1);
        assert_eq!(stats.unreachable_blocks_pruned, 1);
        assert!(matches!(
            program.functions[0].blocks[0].terminator,
            IrTerminator::Jump(1)
        ));
    }

    #[test]
    fn dead_code_elimination_clears_unreachable_blocks() {
        let mut program = test_program(
            vec![
                IrBlock {
                    id: 0,
                    name: "entry".to_string(),
                    instructions: Vec::new(),
                    terminator: IrTerminator::Jump(1),
                },
                IrBlock {
                    id: 1,
                    name: "live".to_string(),
                    instructions: Vec::new(),
                    terminator: IrTerminator::Return(Some(Operand::Constant(1))),
                },
                IrBlock {
                    id: 2,
                    name: "dead".to_string(),
                    instructions: vec![IrInstr::Copy {
                        dst: 0,
                        src: Operand::Constant(3),
                    }],
                    terminator: IrTerminator::Return(Some(Operand::Temp(0))),
                },
            ],
            vec![u8_ty()],
        );

        let stats = dead_code_elimination(&mut program);

        assert_eq!(stats.unreachable_blocks_cleared, 1);
        assert_eq!(stats.instructions_removed, 1);
        assert!(matches!(
            program.functions[0].blocks[2].terminator,
            IrTerminator::Unreachable
        ));
        assert!(program.functions[0].blocks[2].instructions.is_empty());
    }

    #[test]
    fn compact_temps_remaps_surviving_slots() {
        let mut program = test_program(
            vec![IrBlock {
                id: 0,
                name: "entry".to_string(),
                instructions: vec![
                    IrInstr::Copy {
                        dst: 0,
                        src: Operand::Constant(2),
                    },
                    IrInstr::Binary {
                        dst: 2,
                        op: BinaryOp::Add,
                        lhs: Operand::Temp(0),
                        rhs: Operand::Constant(3),
                    },
                ],
                terminator: IrTerminator::Return(Some(Operand::Temp(2))),
            }],
            vec![u8_ty(), u8_ty(), u8_ty()],
        );

        let stats = compact_temps(&mut program);
        let function = &program.functions[0];

        assert_eq!(stats.temp_slots_removed, 1);
        assert_eq!(function.temp_types.len(), 2);
        assert!(matches!(
            function.blocks[0].instructions[1],
            IrInstr::Binary {
                dst: 1,
                lhs: Operand::Temp(0),
                rhs: Operand::Constant(3),
                ..
            }
        ));
        assert!(matches!(
            function.blocks[0].terminator,
            IrTerminator::Return(Some(Operand::Temp(1)))
        ));
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

        let stats = constant_fold(&mut program);
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
        assert_eq!(stats.branches_simplified, 1);
        assert!(matches!(
            program.functions[0].blocks[0].terminator,
            IrTerminator::Jump(1)
        ));
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
