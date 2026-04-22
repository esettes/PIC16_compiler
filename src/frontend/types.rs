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
    pub const fn new(scalar: ScalarType) -> Self {
        Self {
            scalar,
            qualifiers: Qualifiers {
                is_const: false,
                is_volatile: false,
            },
        }
    }

    pub const fn with_qualifiers(mut self, qualifiers: Qualifiers) -> Self {
        self.qualifiers = qualifiers;
        self
    }

    pub fn is_void(self) -> bool {
        self.scalar == ScalarType::Void
    }

    pub fn is_supported_codegen_scalar(self) -> bool {
        matches!(self.scalar, ScalarType::Void | ScalarType::I8 | ScalarType::U8)
    }

    pub fn bit_width(self) -> usize {
        match self.scalar {
            ScalarType::Void => 0,
            ScalarType::I8 | ScalarType::U8 => 8,
            ScalarType::I16 | ScalarType::U16 => 16,
        }
    }
}

impl Default for Type {
    fn default() -> Self {
        Self::new(ScalarType::I16)
    }
}

impl Display for Type {
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
