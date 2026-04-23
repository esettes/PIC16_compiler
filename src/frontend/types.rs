use std::fmt::{Display, Formatter};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ScalarType {
    Void,
    I8,
    U8,
    I16,
    U16,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Qualifiers {
    pub is_const: bool,
    pub is_volatile: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CastKind {
    ZeroExtend,
    SignExtend,
    Truncate,
    Bitcast,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StorageClass {
    Auto,
    Static,
    Extern,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Type {
    pub scalar: ScalarType,
    pub qualifiers: Qualifiers,
}

impl Type {
    /// Creates an unqualified scalar type used throughout the frontend and IR.
    pub const fn new(scalar: ScalarType) -> Self {
        Self {
            scalar,
            qualifiers: Qualifiers {
                is_const: false,
                is_volatile: false,
            },
        }
    }

    /// Returns a copy of the type with the provided qualifiers applied.
    pub const fn with_qualifiers(mut self, qualifiers: Qualifiers) -> Self {
        self.qualifiers = qualifiers;
        self
    }

    /// Returns true when this type is `void`.
    pub fn is_void(self) -> bool {
        self.scalar == ScalarType::Void
    }

    /// Returns true when the backend can currently lower this scalar type.
    pub fn is_supported_codegen_scalar(self) -> bool {
        matches!(
            self.scalar,
            ScalarType::Void | ScalarType::I8 | ScalarType::U8 | ScalarType::I16 | ScalarType::U16
        )
    }

    /// Returns true when the type participates in integer expressions.
    pub fn is_integer(self) -> bool {
        !self.is_void()
    }

    /// Returns true when the scalar uses signed arithmetic semantics.
    pub fn is_signed(self) -> bool {
        matches!(self.scalar, ScalarType::I8 | ScalarType::I16)
    }

    /// Returns true when the scalar uses unsigned arithmetic semantics.
    pub fn is_unsigned(self) -> bool {
        matches!(self.scalar, ScalarType::U8 | ScalarType::U16)
    }

    /// Returns the integer bit width associated with this scalar type.
    pub fn bit_width(self) -> usize {
        match self.scalar {
            ScalarType::Void => 0,
            ScalarType::I8 | ScalarType::U8 => 8,
            ScalarType::I16 | ScalarType::U16 => 16,
        }
    }

    /// Returns the byte width needed to store this scalar in memory.
    pub fn byte_width(self) -> usize {
        self.bit_width().div_ceil(8)
    }

    /// Returns the truncation mask that matches this scalar's bit width.
    pub fn mask(self) -> i64 {
        match self.bit_width() {
            0 => 0,
            8 => 0x00FF,
            16 => 0xFFFF,
            _ => unreachable!("supported widths"),
        }
    }
}

impl Default for Type {
    /// Uses signed 16-bit `int` as the default scalar type.
    fn default() -> Self {
        Self::new(ScalarType::I16)
    }
}

impl Display for Type {
    /// Formats the type as user-facing C syntax including qualifiers.
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        if self.qualifiers.is_const {
            formatter.write_str("const ")?;
        }
        if self.qualifiers.is_volatile {
            formatter.write_str("volatile ")?;
        }
        let name = match self.scalar {
            ScalarType::Void => "void",
            ScalarType::I8 => "char",
            ScalarType::U8 => "unsigned char",
            ScalarType::I16 => "int",
            ScalarType::U16 => "unsigned int",
        };
        formatter.write_str(name)
    }
}

#[cfg(test)]
mod tests {
    use super::{ScalarType, Type};

    #[test]
    /// Confirms the supported integer widths and masks match Phase 2 expectations.
    fn phase_two_integer_types_report_expected_widths() {
        assert_eq!(Type::new(ScalarType::I8).byte_width(), 1);
        assert_eq!(Type::new(ScalarType::U8).mask(), 0x00FF);
        assert_eq!(Type::new(ScalarType::I16).byte_width(), 2);
        assert_eq!(Type::new(ScalarType::U16).mask(), 0xFFFF);
    }

    #[test]
    /// Verifies signedness helpers line up with the declared scalar variants.
    fn signedness_helpers_match_supported_scalars() {
        assert!(Type::new(ScalarType::I16).is_signed());
        assert!(Type::new(ScalarType::U16).is_unsigned());
        assert!(Type::new(ScalarType::U16).is_integer());
        assert!(!Type::new(ScalarType::Void).is_integer());
    }
}
