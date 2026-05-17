// SPDX-License-Identifier: GPL-3.0-or-later

use crate::frontend::ast::BinaryOp;
use crate::frontend::types::{ScalarType, Type};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum RuntimeProfile {
    Small,
    #[default]
    Balanced,
    Fast,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum RuntimeHelperCategory {
    Integer,
    Fixed,
    Float,
    Conversion,
    Shift,
    Division,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum RuntimeHelper {
    MulU8,
    MulI8,
    MulU16,
    MulI16,
    MulU32,
    MulI32,
    MulQ8_8,
    MulUQ8_8,
    MulQ16_16,
    MulUQ16_16,
    DivU8,
    DivI8,
    DivU16,
    DivI16,
    DivU32,
    DivI32,
    DivQ8_8,
    DivUQ8_8,
    DivQ16_16,
    DivUQ16_16,
    ModU8,
    ModI8,
    ModU16,
    ModI16,
    ModU32,
    ModI32,
    Shl8,
    Shl16,
    Shl32,
    ShrU8,
    ShrI8,
    ShrU16,
    ShrI16,
    ShrU32,
    ShrI32,
    F32Add,
    F32Sub,
    F32Mul,
    F32Div,
    F32Cmp,
    F32ToQ16,
    Q16ToF32,
    I32ToF32,
    U32ToF32,
    F32ToI32,
    F32ToU32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimeHelperInfo {
    pub label: &'static str,
    pub operand_ty: Type,
    pub arg_bytes: u16,
    pub local_bytes: u16,
    pub frame_bytes: u16,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimeHelperCatalogEntry {
    pub helper: RuntimeHelper,
    pub label: &'static str,
    pub category: RuntimeHelperCategory,
    pub required_by: &'static str,
    pub arg_bytes: u16,
    pub local_bytes: u16,
    pub frame_bytes: u16,
    pub estimated_words: u16,
    pub dependencies: &'static [RuntimeHelper],
    pub page_sensitive: bool,
    pub target_constraints: &'static str,
}

impl RuntimeHelper {
    pub const ALL: &'static [RuntimeHelper] = &[
        RuntimeHelper::MulU8,
        RuntimeHelper::MulI8,
        RuntimeHelper::MulU16,
        RuntimeHelper::MulI16,
        RuntimeHelper::MulU32,
        RuntimeHelper::MulI32,
        RuntimeHelper::MulQ8_8,
        RuntimeHelper::MulUQ8_8,
        RuntimeHelper::MulQ16_16,
        RuntimeHelper::MulUQ16_16,
        RuntimeHelper::DivU8,
        RuntimeHelper::DivI8,
        RuntimeHelper::DivU16,
        RuntimeHelper::DivI16,
        RuntimeHelper::DivU32,
        RuntimeHelper::DivI32,
        RuntimeHelper::DivQ8_8,
        RuntimeHelper::DivUQ8_8,
        RuntimeHelper::DivQ16_16,
        RuntimeHelper::DivUQ16_16,
        RuntimeHelper::ModU8,
        RuntimeHelper::ModI8,
        RuntimeHelper::ModU16,
        RuntimeHelper::ModI16,
        RuntimeHelper::ModU32,
        RuntimeHelper::ModI32,
        RuntimeHelper::Shl8,
        RuntimeHelper::Shl16,
        RuntimeHelper::Shl32,
        RuntimeHelper::ShrU8,
        RuntimeHelper::ShrI8,
        RuntimeHelper::ShrU16,
        RuntimeHelper::ShrI16,
        RuntimeHelper::ShrU32,
        RuntimeHelper::ShrI32,
        RuntimeHelper::F32Add,
        RuntimeHelper::F32Sub,
        RuntimeHelper::F32Mul,
        RuntimeHelper::F32Div,
        RuntimeHelper::F32Cmp,
        RuntimeHelper::F32ToQ16,
        RuntimeHelper::Q16ToF32,
        RuntimeHelper::I32ToF32,
        RuntimeHelper::U32ToF32,
        RuntimeHelper::F32ToI32,
        RuntimeHelper::F32ToU32,
    ];

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
            Self::MulU32 => RuntimeHelperInfo {
                label: "__rt_mul_u32",
                operand_ty: Type::new(ScalarType::U32),
                arg_bytes: 8,
                local_bytes: 6,
                frame_bytes: 8,
            },
            Self::MulI32 => RuntimeHelperInfo {
                label: "__rt_mul_i32",
                operand_ty: Type::new(ScalarType::I32),
                arg_bytes: 8,
                local_bytes: 6,
                frame_bytes: 8,
            },
            Self::MulQ8_8 => RuntimeHelperInfo {
                label: "__rt_mul_q8_8",
                operand_ty: Type::new(ScalarType::Q8_8),
                arg_bytes: 4,
                local_bytes: 14,
                frame_bytes: 16,
            },
            Self::MulUQ8_8 => RuntimeHelperInfo {
                label: "__rt_mul_uq8_8",
                operand_ty: Type::new(ScalarType::UQ8_8),
                arg_bytes: 4,
                local_bytes: 14,
                frame_bytes: 16,
            },
            Self::MulQ16_16 => RuntimeHelperInfo {
                label: "__rt_mul_q16_16",
                operand_ty: Type::new(ScalarType::Q16_16),
                arg_bytes: 8,
                local_bytes: 16,
                frame_bytes: 18,
            },
            Self::MulUQ16_16 => RuntimeHelperInfo {
                label: "__rt_mul_uq16_16",
                operand_ty: Type::new(ScalarType::UQ16_16),
                arg_bytes: 8,
                local_bytes: 16,
                frame_bytes: 18,
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
            Self::DivU32 => RuntimeHelperInfo {
                label: "__rt_div_u32",
                operand_ty: Type::new(ScalarType::U32),
                arg_bytes: 8,
                local_bytes: 6,
                frame_bytes: 8,
            },
            Self::DivI32 => RuntimeHelperInfo {
                label: "__rt_div_i32",
                operand_ty: Type::new(ScalarType::I32),
                arg_bytes: 8,
                local_bytes: 6,
                frame_bytes: 8,
            },
            Self::DivQ8_8 => RuntimeHelperInfo {
                label: "__rt_div_q8_8",
                operand_ty: Type::new(ScalarType::Q8_8),
                arg_bytes: 4,
                local_bytes: 14,
                frame_bytes: 16,
            },
            Self::DivUQ8_8 => RuntimeHelperInfo {
                label: "__rt_div_uq8_8",
                operand_ty: Type::new(ScalarType::UQ8_8),
                arg_bytes: 4,
                local_bytes: 14,
                frame_bytes: 16,
            },
            Self::DivQ16_16 => RuntimeHelperInfo {
                label: "__rt_div_q16_16",
                operand_ty: Type::new(ScalarType::Q16_16),
                arg_bytes: 8,
                local_bytes: 14,
                frame_bytes: 16,
            },
            Self::DivUQ16_16 => RuntimeHelperInfo {
                label: "__rt_div_uq16_16",
                operand_ty: Type::new(ScalarType::UQ16_16),
                arg_bytes: 8,
                local_bytes: 14,
                frame_bytes: 16,
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
            Self::ModU32 => RuntimeHelperInfo {
                label: "__rt_mod_u32",
                operand_ty: Type::new(ScalarType::U32),
                arg_bytes: 8,
                local_bytes: 6,
                frame_bytes: 8,
            },
            Self::ModI32 => RuntimeHelperInfo {
                label: "__rt_mod_i32",
                operand_ty: Type::new(ScalarType::I32),
                arg_bytes: 8,
                local_bytes: 6,
                frame_bytes: 8,
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
            Self::Shl32 => RuntimeHelperInfo {
                label: "__rt_shl32",
                operand_ty: Type::new(ScalarType::U32),
                arg_bytes: 8,
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
            Self::ShrU32 => RuntimeHelperInfo {
                label: "__rt_shr_u32",
                operand_ty: Type::new(ScalarType::U32),
                arg_bytes: 8,
                local_bytes: 0,
                frame_bytes: 2,
            },
            Self::ShrI32 => RuntimeHelperInfo {
                label: "__rt_shr_i32",
                operand_ty: Type::new(ScalarType::I32),
                arg_bytes: 8,
                local_bytes: 0,
                frame_bytes: 2,
            },
            Self::F32Add => RuntimeHelperInfo {
                label: "__rt_f32_add",
                operand_ty: Type::new(ScalarType::F32),
                arg_bytes: 8,
                local_bytes: 16,
                frame_bytes: 18,
            },
            Self::F32Sub => RuntimeHelperInfo {
                label: "__rt_f32_sub",
                operand_ty: Type::new(ScalarType::F32),
                arg_bytes: 8,
                local_bytes: 16,
                frame_bytes: 18,
            },
            Self::F32Mul => RuntimeHelperInfo {
                label: "__rt_f32_mul",
                operand_ty: Type::new(ScalarType::F32),
                arg_bytes: 8,
                local_bytes: 16,
                frame_bytes: 18,
            },
            Self::F32Div => RuntimeHelperInfo {
                label: "__rt_f32_div",
                operand_ty: Type::new(ScalarType::F32),
                arg_bytes: 8,
                local_bytes: 16,
                frame_bytes: 18,
            },
            Self::F32Cmp => RuntimeHelperInfo {
                label: "__rt_f32_cmp",
                operand_ty: Type::new(ScalarType::F32),
                arg_bytes: 8,
                local_bytes: 16,
                frame_bytes: 18,
            },
            Self::F32ToQ16 => RuntimeHelperInfo {
                label: "__rt_f32_to_q16_16",
                operand_ty: Type::new(ScalarType::F32),
                arg_bytes: 4,
                local_bytes: 16,
                frame_bytes: 18,
            },
            Self::Q16ToF32 => RuntimeHelperInfo {
                label: "__rt_q16_16_to_f32",
                operand_ty: Type::new(ScalarType::Q16_16),
                arg_bytes: 4,
                local_bytes: 16,
                frame_bytes: 18,
            },
            Self::I32ToF32 => RuntimeHelperInfo {
                label: "__rt_i32_to_f32",
                operand_ty: Type::new(ScalarType::I32),
                arg_bytes: 4,
                local_bytes: 16,
                frame_bytes: 18,
            },
            Self::U32ToF32 => RuntimeHelperInfo {
                label: "__rt_u32_to_f32",
                operand_ty: Type::new(ScalarType::U32),
                arg_bytes: 4,
                local_bytes: 16,
                frame_bytes: 18,
            },
            Self::F32ToI32 => RuntimeHelperInfo {
                label: "__rt_f32_to_i32",
                operand_ty: Type::new(ScalarType::F32),
                arg_bytes: 4,
                local_bytes: 16,
                frame_bytes: 18,
            },
            Self::F32ToU32 => RuntimeHelperInfo {
                label: "__rt_f32_to_u32",
                operand_ty: Type::new(ScalarType::F32),
                arg_bytes: 4,
                local_bytes: 16,
                frame_bytes: 18,
            },
        }
    }

    pub const fn label(self) -> &'static str {
        self.info().label
    }

    pub const fn category(self) -> RuntimeHelperCategory {
        match self {
            Self::MulQ8_8
            | Self::MulUQ8_8
            | Self::MulQ16_16
            | Self::MulUQ16_16
            | Self::DivQ8_8
            | Self::DivUQ8_8
            | Self::DivQ16_16
            | Self::DivUQ16_16 => RuntimeHelperCategory::Fixed,
            Self::F32Add | Self::F32Sub | Self::F32Mul | Self::F32Div | Self::F32Cmp => {
                RuntimeHelperCategory::Float
            }
            Self::F32ToQ16
            | Self::Q16ToF32
            | Self::I32ToF32
            | Self::U32ToF32
            | Self::F32ToI32
            | Self::F32ToU32 => RuntimeHelperCategory::Conversion,
            Self::Shl8
            | Self::Shl16
            | Self::Shl32
            | Self::ShrU8
            | Self::ShrI8
            | Self::ShrU16
            | Self::ShrI16
            | Self::ShrU32
            | Self::ShrI32 => RuntimeHelperCategory::Shift,
            Self::DivU8
            | Self::DivI8
            | Self::DivU16
            | Self::DivI16
            | Self::DivU32
            | Self::DivI32
            | Self::ModU8
            | Self::ModI8
            | Self::ModU16
            | Self::ModI16
            | Self::ModU32
            | Self::ModI32 => RuntimeHelperCategory::Division,
            _ => RuntimeHelperCategory::Integer,
        }
    }

    pub const fn required_by(self) -> &'static str {
        match self.category() {
            RuntimeHelperCategory::Integer => "integer arithmetic",
            RuntimeHelperCategory::Fixed => "fixed-point arithmetic",
            RuntimeHelperCategory::Float => "finite f32 arithmetic/comparison",
            RuntimeHelperCategory::Conversion => "numeric conversion",
            RuntimeHelperCategory::Shift => "dynamic shift",
            RuntimeHelperCategory::Division => "integer division/modulo",
        }
    }

    pub const fn dependencies(self) -> &'static [RuntimeHelper] {
        &[]
    }

    pub const fn estimated_words(self) -> u16 {
        let info = self.info();
        info.arg_bytes + info.local_bytes + info.frame_bytes + 24
    }

    pub const fn page_sensitive(self) -> bool {
        true
    }

    pub const fn target_constraints(self) -> &'static str {
        match self.category() {
            RuntimeHelperCategory::Float => "large on PIC16F628A; prefer PIC16F877A or fixed-point",
            RuntimeHelperCategory::Fixed => "Q16.16 helpers may be large on small targets",
            _ => "normal program-memory and stack limits",
        }
    }

    pub const fn catalog_entry(self) -> RuntimeHelperCatalogEntry {
        let info = self.info();
        RuntimeHelperCatalogEntry {
            helper: self,
            label: info.label,
            category: self.category(),
            required_by: self.required_by(),
            arg_bytes: info.arg_bytes,
            local_bytes: info.local_bytes,
            frame_bytes: info.frame_bytes,
            estimated_words: self.estimated_words(),
            dependencies: self.dependencies(),
            page_sensitive: self.page_sensitive(),
            target_constraints: self.target_constraints(),
        }
    }
}

pub fn runtime_helper_by_label(label: &str) -> Option<RuntimeHelper> {
    RuntimeHelper::ALL
        .iter()
        .copied()
        .find(|helper| helper.label() == label)
}

pub fn validate_helper_dependency_graph(helpers: &[RuntimeHelper]) -> Result<(), String> {
    for helper in helpers {
        for dependency in helper.dependencies() {
            if !RuntimeHelper::ALL.contains(dependency) {
                return Err(format!(
                    "unknown runtime helper dependency `{}` required by `{}`",
                    dependency.label(),
                    helper.label()
                ));
            }
            if dependency == helper {
                return Err(format!(
                    "runtime helper dependency cycle at `{}`",
                    helper.label()
                ));
            }
        }
    }
    Ok(())
}

impl RuntimeHelperCategory {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Integer => "integer",
            Self::Fixed => "fixed",
            Self::Float => "float",
            Self::Conversion => "conversion",
            Self::Shift => "shift",
            Self::Division => "division",
        }
    }
}

pub fn binary_helper(op: BinaryOp, ty: Type) -> Option<RuntimeHelper> {
    if !ty.is_integer() && !ty.is_fixed() && !ty.is_float() {
        return None;
    }

    match (op, ty.scalar) {
        (BinaryOp::Add, ScalarType::F32) => Some(RuntimeHelper::F32Add),
        (BinaryOp::Sub, ScalarType::F32) => Some(RuntimeHelper::F32Sub),
        (BinaryOp::Multiply, ScalarType::F32) => Some(RuntimeHelper::F32Mul),
        (BinaryOp::Divide, ScalarType::F32) => Some(RuntimeHelper::F32Div),
        (BinaryOp::Multiply, ScalarType::U8) => Some(RuntimeHelper::MulU8),
        (BinaryOp::Multiply, ScalarType::I8) => Some(RuntimeHelper::MulI8),
        (BinaryOp::Multiply, ScalarType::U16) => Some(RuntimeHelper::MulU16),
        (BinaryOp::Multiply, ScalarType::I16) => Some(RuntimeHelper::MulI16),
        (BinaryOp::Multiply, ScalarType::U32) => Some(RuntimeHelper::MulU32),
        (BinaryOp::Multiply, ScalarType::I32) => Some(RuntimeHelper::MulI32),
        (BinaryOp::Multiply, ScalarType::Q8_8) => Some(RuntimeHelper::MulQ8_8),
        (BinaryOp::Multiply, ScalarType::UQ8_8) => Some(RuntimeHelper::MulUQ8_8),
        (BinaryOp::Multiply, ScalarType::Q16_16) => Some(RuntimeHelper::MulQ16_16),
        (BinaryOp::Multiply, ScalarType::UQ16_16) => Some(RuntimeHelper::MulUQ16_16),
        (BinaryOp::Divide, ScalarType::U8) => Some(RuntimeHelper::DivU8),
        (BinaryOp::Divide, ScalarType::I8) => Some(RuntimeHelper::DivI8),
        (BinaryOp::Divide, ScalarType::U16) => Some(RuntimeHelper::DivU16),
        (BinaryOp::Divide, ScalarType::I16) => Some(RuntimeHelper::DivI16),
        (BinaryOp::Divide, ScalarType::U32) => Some(RuntimeHelper::DivU32),
        (BinaryOp::Divide, ScalarType::I32) => Some(RuntimeHelper::DivI32),
        (BinaryOp::Divide, ScalarType::Q8_8) => Some(RuntimeHelper::DivQ8_8),
        (BinaryOp::Divide, ScalarType::UQ8_8) => Some(RuntimeHelper::DivUQ8_8),
        (BinaryOp::Divide, ScalarType::Q16_16) => Some(RuntimeHelper::DivQ16_16),
        (BinaryOp::Divide, ScalarType::UQ16_16) => Some(RuntimeHelper::DivUQ16_16),
        (BinaryOp::Modulo, ScalarType::U8) => Some(RuntimeHelper::ModU8),
        (BinaryOp::Modulo, ScalarType::I8) => Some(RuntimeHelper::ModI8),
        (BinaryOp::Modulo, ScalarType::U16) => Some(RuntimeHelper::ModU16),
        (BinaryOp::Modulo, ScalarType::I16) => Some(RuntimeHelper::ModI16),
        (BinaryOp::Modulo, ScalarType::U32) => Some(RuntimeHelper::ModU32),
        (BinaryOp::Modulo, ScalarType::I32) => Some(RuntimeHelper::ModI32),
        (BinaryOp::ShiftLeft, ScalarType::U8 | ScalarType::I8) => Some(RuntimeHelper::Shl8),
        (BinaryOp::ShiftLeft, ScalarType::U16 | ScalarType::I16) => Some(RuntimeHelper::Shl16),
        (BinaryOp::ShiftLeft, ScalarType::U32 | ScalarType::I32) => Some(RuntimeHelper::Shl32),
        (BinaryOp::ShiftRight, ScalarType::U8) => Some(RuntimeHelper::ShrU8),
        (BinaryOp::ShiftRight, ScalarType::I8) => Some(RuntimeHelper::ShrI8),
        (BinaryOp::ShiftRight, ScalarType::U16) => Some(RuntimeHelper::ShrU16),
        (BinaryOp::ShiftRight, ScalarType::I16) => Some(RuntimeHelper::ShrI16),
        (BinaryOp::ShiftRight, ScalarType::U32) => Some(RuntimeHelper::ShrU32),
        (BinaryOp::ShiftRight, ScalarType::I32) => Some(RuntimeHelper::ShrI32),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{RuntimeHelper, binary_helper};
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
