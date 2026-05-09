// SPDX-License-Identifier: GPL-3.0-or-later

use crate::frontend::ast::{BinaryOp, UnaryOp};
use crate::frontend::types::{IntegerSuffix, ScalarType, Type};

/// Infers the smallest integer type that can represent an unsuffixed literal value.
pub fn infer_integer_literal_type(value: i64) -> Type {
    infer_integer_literal_type_with_suffix(value, IntegerSuffix::None)
}

/// Infers the Phase 21 integer literal type, honoring supported `U`, `L`, and `UL` suffixes.
pub fn infer_integer_literal_type_with_suffix(value: i64, suffix: IntegerSuffix) -> Type {
    match suffix {
        IntegerSuffix::Long => Type::new(ScalarType::I32),
        IntegerSuffix::UnsignedLong => Type::new(ScalarType::U32),
        IntegerSuffix::Unsigned => {
            if value <= i64::from(u16::MAX) {
                Type::new(ScalarType::U16)
            } else {
                Type::new(ScalarType::U32)
            }
        }
        IntegerSuffix::None => {
            if value <= i64::from(i16::MAX) {
                Type::new(ScalarType::I16)
            } else if value <= i64::from(u16::MAX) {
                Type::new(ScalarType::U16)
            } else if value <= i64::from(i32::MAX) {
                Type::new(ScalarType::I32)
            } else {
                Type::new(ScalarType::U32)
            }
        }
    }
}

/// Masks a value to the bit width of the requested scalar type.
pub fn normalize_value(value: i64, ty: Type) -> i64 {
    if ty.is_void() {
        return value;
    }
    value & ty.mask()
}

/// Interprets a masked integer value using the signedness of the given type.
pub fn signed_value(value: i64, ty: Type) -> i64 {
    let normalized = normalize_value(value, ty);
    if !ty.is_signed() || ty.bit_width() == 0 {
        return normalized;
    }

    let sign_bit = 1_i64 << (ty.bit_width() - 1);
    if (normalized & sign_bit) != 0 {
        normalized - (1_i64 << ty.bit_width())
    } else {
        normalized
    }
}

/// Extracts the low byte of a value after type-aware normalization.
pub fn low_byte(value: i64, ty: Type) -> u8 {
    (normalize_value(value, ty) & 0xFF) as u8
}

/// Extracts the high byte of a 16-bit value after type-aware normalization.
pub fn high_byte(value: i64, ty: Type) -> u8 {
    ((normalize_value(value, ty) >> 8) & 0xFF) as u8
}

/// Extracts one little-endian byte of a value after type-aware normalization.
pub fn value_byte(value: i64, ty: Type, byte_index: usize) -> u8 {
    ((normalize_value(value, ty) >> (8 * byte_index)) & 0xFF) as u8
}

/// Evaluates a unary integer operation using Phase 2 width and signedness rules.
pub fn eval_unary(op: UnaryOp, value: i64, operand_ty: Type, result_ty: Type) -> i64 {
    let operand = normalize_value(value, operand_ty);
    match op {
        UnaryOp::Negate => normalize_value(-signed_value(operand, operand_ty), result_ty),
        UnaryOp::LogicalNot => i64::from(normalize_value(operand, operand_ty) == 0),
        UnaryOp::BitwiseNot => normalize_value(!operand, result_ty),
    }
}

/// Evaluates an integer binary operation with PIC16-compatible width truncation.
pub fn eval_binary(op: BinaryOp, lhs: i64, rhs: i64, operand_ty: Type, result_ty: Type) -> i64 {
    let lhs_unsigned = normalize_value(lhs, operand_ty);
    let rhs_unsigned = normalize_value(rhs, operand_ty);
    let lhs_signed = signed_value(lhs, operand_ty);
    let rhs_signed = signed_value(rhs, operand_ty);
    let shift_count = (rhs_unsigned as usize).min(operand_ty.bit_width());

    let value = match op {
        BinaryOp::Add => i128::from(lhs_unsigned) + i128::from(rhs_unsigned),
        BinaryOp::Sub => i128::from(lhs_unsigned) - i128::from(rhs_unsigned),
        BinaryOp::Multiply => i128::from(lhs_unsigned) * i128::from(rhs_unsigned),
        BinaryOp::Divide => {
            if rhs_unsigned == 0 {
                0_i128
            } else if operand_ty.is_signed() {
                i128::from(lhs_signed / rhs_signed)
            } else {
                i128::from(lhs_unsigned / rhs_unsigned)
            }
        }
        BinaryOp::Modulo => {
            if rhs_unsigned == 0 {
                0_i128
            } else if operand_ty.is_signed() {
                i128::from(lhs_signed % rhs_signed)
            } else {
                i128::from(lhs_unsigned % rhs_unsigned)
            }
        }
        BinaryOp::ShiftLeft => i128::from(lhs_unsigned) << shift_count,
        BinaryOp::ShiftRight => {
            if operand_ty.is_signed() {
                i128::from(lhs_signed >> shift_count)
            } else {
                i128::from(lhs_unsigned >> shift_count)
            }
        }
        BinaryOp::BitAnd => i128::from(lhs_unsigned & rhs_unsigned),
        BinaryOp::BitOr => i128::from(lhs_unsigned | rhs_unsigned),
        BinaryOp::BitXor => i128::from(lhs_unsigned ^ rhs_unsigned),
        BinaryOp::LogicalAnd => i128::from(u8::from(lhs_unsigned != 0 && rhs_unsigned != 0)),
        BinaryOp::LogicalOr => i128::from(u8::from(lhs_unsigned != 0 || rhs_unsigned != 0)),
        BinaryOp::Equal => i128::from(u8::from(lhs_unsigned == rhs_unsigned)),
        BinaryOp::NotEqual => i128::from(u8::from(lhs_unsigned != rhs_unsigned)),
        BinaryOp::Less => i128::from(u8::from(compare_rel(op, lhs, rhs, operand_ty))),
        BinaryOp::LessEqual => i128::from(u8::from(compare_rel(op, lhs, rhs, operand_ty))),
        BinaryOp::Greater => i128::from(u8::from(compare_rel(op, lhs, rhs, operand_ty))),
        BinaryOp::GreaterEqual => i128::from(u8::from(compare_rel(op, lhs, rhs, operand_ty))),
    };

    normalize_i128(value, result_ty)
}

fn normalize_i128(value: i128, ty: Type) -> i64 {
    if ty.is_void() {
        return value as i64;
    }
    (value & i128::from(ty.mask())) as i64
}

/// Applies signed or unsigned relational semantics for the supplied operand type.
pub fn compare_rel(op: BinaryOp, lhs: i64, rhs: i64, ty: Type) -> bool {
    let lhs_unsigned = normalize_value(lhs, ty);
    let rhs_unsigned = normalize_value(rhs, ty);
    let lhs_signed = signed_value(lhs, ty);
    let rhs_signed = signed_value(rhs, ty);

    match op {
        BinaryOp::Equal => lhs_unsigned == rhs_unsigned,
        BinaryOp::NotEqual => lhs_unsigned != rhs_unsigned,
        BinaryOp::Less => {
            if ty.is_signed() {
                lhs_signed < rhs_signed
            } else {
                lhs_unsigned < rhs_unsigned
            }
        }
        BinaryOp::LessEqual => {
            if ty.is_signed() {
                lhs_signed <= rhs_signed
            } else {
                lhs_unsigned <= rhs_unsigned
            }
        }
        BinaryOp::Greater => {
            if ty.is_signed() {
                lhs_signed > rhs_signed
            } else {
                lhs_unsigned > rhs_unsigned
            }
        }
        BinaryOp::GreaterEqual => {
            if ty.is_signed() {
                lhs_signed >= rhs_signed
            } else {
                lhs_unsigned >= rhs_unsigned
            }
        }
        _ => unreachable!("relational operator"),
    }
}

#[cfg(test)]
mod tests {
    use super::{compare_rel, eval_binary, eval_unary, infer_integer_literal_type, signed_value};
    use crate::frontend::ast::{BinaryOp, UnaryOp};
    use crate::frontend::types::{ScalarType, Type};

    #[test]
    /// Confirms literal inference selects signed or unsigned 16-bit types as expected.
    fn infers_literal_types_for_phase_two() {
        assert_eq!(infer_integer_literal_type(0).scalar, ScalarType::I16);
        assert_eq!(infer_integer_literal_type(255).scalar, ScalarType::I16);
        assert_eq!(infer_integer_literal_type(32_767).scalar, ScalarType::I16);
        assert_eq!(infer_integer_literal_type(32_768).scalar, ScalarType::U16);
        assert_eq!(infer_integer_literal_type(70_000).scalar, ScalarType::I32);
        assert_eq!(infer_integer_literal_type(3_000_000_000).scalar, ScalarType::U32);
    }

    #[test]
    /// Checks that signed and unsigned relations diverge on the same bit pattern.
    fn evaluates_signed_and_unsigned_relations() {
        let signed = Type::new(ScalarType::I16);
        let unsigned = Type::new(ScalarType::U16);

        assert!(compare_rel(BinaryOp::Less, -1, 1, signed));
        assert!(!compare_rel(BinaryOp::Less, -1, 1, unsigned));
        assert!(compare_rel(BinaryOp::GreaterEqual, 0xFFFF, 1, unsigned));
        assert!(!compare_rel(BinaryOp::GreaterEqual, -2, 1, signed));
    }

    #[test]
    /// Verifies representative 16-bit arithmetic and unary operations wrap correctly.
    fn evaluates_16bit_binary_and_unary_values() {
        let u16_ty = Type::new(ScalarType::U16);
        let i16_ty = Type::new(ScalarType::I16);

        assert_eq!(eval_binary(BinaryOp::Add, 0xFFFF, 1, u16_ty, u16_ty), 0);
        assert_eq!(eval_binary(BinaryOp::Sub, 0, 1, u16_ty, u16_ty), 0xFFFF);
        assert_eq!(eval_binary(BinaryOp::ShiftLeft, 1, 3, u16_ty, u16_ty), 8);
        assert_eq!(eval_binary(BinaryOp::ShiftRight, 0x8000, 15, u16_ty, u16_ty), 1);
        assert_eq!(eval_unary(UnaryOp::BitwiseNot, 0x00FF, u16_ty, u16_ty), 0xFF00);
        assert_eq!(signed_value(eval_unary(UnaryOp::Negate, 2, i16_ty, i16_ty), i16_ty), -2);
        assert_eq!(signed_value(eval_binary(BinaryOp::ShiftRight, -2, 1, i16_ty, i16_ty), i16_ty), -1);
    }
}
// SPDX-License-Identifier: GPL-3.0-or-later
