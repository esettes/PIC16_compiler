// SPDX-License-Identifier: GPL-3.0-or-later

use crate::frontend::ast::BinaryOp;
use crate::frontend::types::{ScalarType, Type};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum RuntimeHelper {
    MulU8,
    MulI8,
    MulU16,
    MulI16,
    DivU8,
    DivI8,
    DivU16,
    DivI16,
    ModU8,
    ModI8,
    ModU16,
    ModI16,
    Shl8,
    Shl16,
    ShrU8,
    ShrI8,
    ShrU16,
    ShrI16,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimeHelperInfo {
    pub label: &'static str,
    pub operand_ty: Type,
    pub arg_bytes: u16,
    pub local_bytes: u16,
    pub frame_bytes: u16,
}

impl RuntimeHelper {
    pub const fn info(self) -> RuntimeHelperInfo {
        match self {
            Self::MulU8 => RuntimeHelperInfo {
                label: "__rt_mul_u8",
                operand_ty: Type::new(ScalarType::U8),
                arg_bytes: 2,
                local_bytes: 3,
                frame_bytes: 5,
            },
            Self::MulI8 => RuntimeHelperInfo {
                label: "__rt_mul_i8",
                operand_ty: Type::new(ScalarType::I8),
                arg_bytes: 2,
                local_bytes: 3,
                frame_bytes: 5,
            },
            Self::MulU16 => RuntimeHelperInfo {
                label: "__rt_mul_u16",
                operand_ty: Type::new(ScalarType::U16),
                arg_bytes: 4,
                local_bytes: 4,
                frame_bytes: 6,
            },
            Self::MulI16 => RuntimeHelperInfo {
                label: "__rt_mul_i16",
                operand_ty: Type::new(ScalarType::I16),
                arg_bytes: 4,
                local_bytes: 4,
                frame_bytes: 6,
            },
            Self::DivU8 => RuntimeHelperInfo {
                label: "__rt_div_u8",
                operand_ty: Type::new(ScalarType::U8),
                arg_bytes: 2,
                local_bytes: 3,
                frame_bytes: 5,
            },
            Self::DivI8 => RuntimeHelperInfo {
                label: "__rt_div_i8",
                operand_ty: Type::new(ScalarType::I8),
                arg_bytes: 2,
                local_bytes: 3,
                frame_bytes: 5,
            },
            Self::DivU16 => RuntimeHelperInfo {
                label: "__rt_div_u16",
                operand_ty: Type::new(ScalarType::U16),
                arg_bytes: 4,
                local_bytes: 4,
                frame_bytes: 6,
            },
            Self::DivI16 => RuntimeHelperInfo {
                label: "__rt_div_i16",
                operand_ty: Type::new(ScalarType::I16),
                arg_bytes: 4,
                local_bytes: 4,
                frame_bytes: 6,
            },
            Self::ModU8 => RuntimeHelperInfo {
                label: "__rt_mod_u8",
                operand_ty: Type::new(ScalarType::U8),
                arg_bytes: 2,
                local_bytes: 3,
                frame_bytes: 5,
            },
            Self::ModI8 => RuntimeHelperInfo {
                label: "__rt_mod_i8",
                operand_ty: Type::new(ScalarType::I8),
                arg_bytes: 2,
                local_bytes: 3,
                frame_bytes: 5,
            },
            Self::ModU16 => RuntimeHelperInfo {
                label: "__rt_mod_u16",
                operand_ty: Type::new(ScalarType::U16),
                arg_bytes: 4,
                local_bytes: 4,
                frame_bytes: 6,
            },
            Self::ModI16 => RuntimeHelperInfo {
                label: "__rt_mod_i16",
                operand_ty: Type::new(ScalarType::I16),
                arg_bytes: 4,
                local_bytes: 4,
                frame_bytes: 6,
            },
            Self::Shl8 => RuntimeHelperInfo {
                label: "__rt_shl8",
                operand_ty: Type::new(ScalarType::U8),
                arg_bytes: 2,
                local_bytes: 0,
                frame_bytes: 2,
            },
            Self::Shl16 => RuntimeHelperInfo {
                label: "__rt_shl16",
                operand_ty: Type::new(ScalarType::U16),
                arg_bytes: 4,
                local_bytes: 0,
                frame_bytes: 2,
            },
            Self::ShrU8 => RuntimeHelperInfo {
                label: "__rt_shr_u8",
                operand_ty: Type::new(ScalarType::U8),
                arg_bytes: 2,
                local_bytes: 0,
                frame_bytes: 2,
            },
            Self::ShrI8 => RuntimeHelperInfo {
                label: "__rt_shr_i8",
                operand_ty: Type::new(ScalarType::I8),
                arg_bytes: 2,
                local_bytes: 0,
                frame_bytes: 2,
            },
            Self::ShrU16 => RuntimeHelperInfo {
                label: "__rt_shr_u16",
                operand_ty: Type::new(ScalarType::U16),
                arg_bytes: 4,
                local_bytes: 0,
                frame_bytes: 2,
            },
            Self::ShrI16 => RuntimeHelperInfo {
                label: "__rt_shr_i16",
                operand_ty: Type::new(ScalarType::I16),
                arg_bytes: 4,
                local_bytes: 0,
                frame_bytes: 2,
            },
        }
    }

    pub const fn label(self) -> &'static str {
        self.info().label
    }
}

pub fn binary_helper(op: BinaryOp, ty: Type) -> Option<RuntimeHelper> {
    if !ty.is_integer() {
        return None;
    }

    match (op, ty.scalar) {
        (BinaryOp::Multiply, ScalarType::U8) => Some(RuntimeHelper::MulU8),
        (BinaryOp::Multiply, ScalarType::I8) => Some(RuntimeHelper::MulI8),
        (BinaryOp::Multiply, ScalarType::U16) => Some(RuntimeHelper::MulU16),
        (BinaryOp::Multiply, ScalarType::I16) => Some(RuntimeHelper::MulI16),
        (BinaryOp::Divide, ScalarType::U8) => Some(RuntimeHelper::DivU8),
        (BinaryOp::Divide, ScalarType::I8) => Some(RuntimeHelper::DivI8),
        (BinaryOp::Divide, ScalarType::U16) => Some(RuntimeHelper::DivU16),
        (BinaryOp::Divide, ScalarType::I16) => Some(RuntimeHelper::DivI16),
        (BinaryOp::Modulo, ScalarType::U8) => Some(RuntimeHelper::ModU8),
        (BinaryOp::Modulo, ScalarType::I8) => Some(RuntimeHelper::ModI8),
        (BinaryOp::Modulo, ScalarType::U16) => Some(RuntimeHelper::ModU16),
        (BinaryOp::Modulo, ScalarType::I16) => Some(RuntimeHelper::ModI16),
        (BinaryOp::ShiftLeft, ScalarType::U8 | ScalarType::I8) => Some(RuntimeHelper::Shl8),
        (BinaryOp::ShiftLeft, ScalarType::U16 | ScalarType::I16) => Some(RuntimeHelper::Shl16),
        (BinaryOp::ShiftRight, ScalarType::U8) => Some(RuntimeHelper::ShrU8),
        (BinaryOp::ShiftRight, ScalarType::I8) => Some(RuntimeHelper::ShrI8),
        (BinaryOp::ShiftRight, ScalarType::U16) => Some(RuntimeHelper::ShrU16),
        (BinaryOp::ShiftRight, ScalarType::I16) => Some(RuntimeHelper::ShrI16),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{binary_helper, RuntimeHelper};
    use crate::frontend::ast::BinaryOp;
    use crate::frontend::types::{ScalarType, Type};

    #[test]
    /// Verifies helper classification distinguishes width and signedness for Phase 5 ops.
    fn classifies_phase_five_runtime_helpers() {
        assert_eq!(
            binary_helper(BinaryOp::Multiply, Type::new(ScalarType::U16)),
            Some(RuntimeHelper::MulU16)
        );
        assert_eq!(
            binary_helper(BinaryOp::Divide, Type::new(ScalarType::I8)),
            Some(RuntimeHelper::DivI8)
        );
        assert_eq!(
            binary_helper(BinaryOp::ShiftRight, Type::new(ScalarType::I16)),
            Some(RuntimeHelper::ShrI16)
        );
    }
}
// SPDX-License-Identifier: GPL-3.0-or-later
