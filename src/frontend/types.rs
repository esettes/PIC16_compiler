// SPDX-License-Identifier: GPL-3.0-or-later

use std::fmt::{Display, Formatter};

pub type StructId = usize;
pub const MAX_POINTER_DEPTH: usize = 8;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum AddressSpace {
    #[default]
    Data,
    Rom,
}

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
    pub address_space: AddressSpace,
    pub qualifiers: Qualifiers,
    pub pointer_depth: u8,
    pub pointer_qualifiers: [Qualifiers; MAX_POINTER_DEPTH],
    pub array_len: Option<usize>,
    pub struct_id: Option<StructId>,
    pub struct_size: usize,
}

impl Type {
    /// Creates an unqualified scalar type used throughout the frontend and IR.
    pub const fn new(scalar: ScalarType) -> Self {
        Self {
            scalar,
            address_space: AddressSpace::Data,
            qualifiers: Qualifiers {
                is_const: false,
                is_volatile: false,
            },
            pointer_depth: 0,
            pointer_qualifiers: [Qualifiers {
                is_const: false,
                is_volatile: false,
            }; MAX_POINTER_DEPTH],
            array_len: None,
            struct_id: None,
            struct_size: 0,
        }
    }

    /// Creates an unqualified named struct type with fixed packed byte size.
    pub const fn struct_type(struct_id: StructId, struct_size: usize) -> Self {
        Self {
            scalar: ScalarType::Void,
            address_space: AddressSpace::Data,
            qualifiers: Qualifiers {
                is_const: false,
                is_volatile: false,
            },
            pointer_depth: 0,
            pointer_qualifiers: [Qualifiers {
                is_const: false,
                is_volatile: false,
            }; MAX_POINTER_DEPTH],
            array_len: None,
            struct_id: Some(struct_id),
            struct_size,
        }
    }

    /// Returns a copy of the type with the provided qualifiers applied.
    pub const fn with_qualifiers(mut self, qualifiers: Qualifiers) -> Self {
        self.qualifiers = qualifiers;
        self
    }

    /// Returns a copy of the type placed in the requested address space.
    pub const fn with_address_space(mut self, address_space: AddressSpace) -> Self {
        self.address_space = address_space;
        self
    }

    /// Returns one otherwise-identical type with `const`/`volatile` qualifiers stripped.
    pub fn unqualified(mut self) -> Self {
        self.qualifiers = Qualifiers {
            is_const: false,
            is_volatile: false,
        };
        for qualifiers in &mut self.pointer_qualifiers {
            *qualifiers = Qualifiers::default();
        }
        self
    }

    /// Returns a pointer type that targets the provided base object type.
    pub const fn pointer_to(mut self) -> Self {
        self = self.pointer_to_with_qualifiers(Qualifiers {
            is_const: false,
            is_volatile: false,
        });
        self
    }

    /// Returns a pointer type with qualifiers applied to the new outer pointer object.
    pub const fn pointer_to_with_qualifiers(mut self, qualifiers: Qualifiers) -> Self {
        if self.pointer_depth < MAX_POINTER_DEPTH as u8 {
            self.pointer_qualifiers[self.pointer_depth as usize] = qualifiers;
            self.pointer_depth += 1;
        }
        self.array_len = None;
        self
    }

    /// Returns a fixed-size one-dimensional array type over the provided element type.
    pub const fn array_of(mut self, len: usize) -> Self {
        self.array_len = Some(len);
        self
    }

    /// Returns the element or pointee type for arrays and pointers.
    pub const fn element_type(mut self) -> Self {
        if self.array_len.is_some() {
            self.array_len = None;
            return self;
        }
        if self.pointer_depth > 0 {
            self.pointer_depth -= 1;
            self.pointer_qualifiers[self.pointer_depth as usize] = Qualifiers {
                is_const: false,
                is_volatile: false,
            };
        }
        self.array_len = None;
        self
    }

    /// Returns the array-decayed pointer form used in value contexts.
    pub const fn decay(self) -> Self {
        self.element_type().pointer_to()
    }

    /// Returns true when this type is plain `void`.
    pub fn is_void(self) -> bool {
        self.scalar == ScalarType::Void
            && self.struct_id.is_none()
            && self.pointer_depth == 0
            && self.array_len.is_none()
    }

    /// Returns true when this type is a complete struct object type.
    pub fn is_struct(self) -> bool {
        self.struct_id.is_some() && self.pointer_depth == 0 && self.array_len.is_none()
    }

    /// Returns true when this type's base object is a struct.
    pub fn has_struct_base(self) -> bool {
        self.struct_id.is_some()
    }

    /// Returns true when this type is a constrained Phase 3 data pointer.
    pub fn is_pointer(self) -> bool {
        self.pointer_depth > 0 && self.array_len.is_none()
    }

    /// Returns true when this type is a fixed-size one-dimensional array.
    pub fn is_array(self) -> bool {
        self.array_len.is_some()
    }

    /// Returns true when this array was declared with `[]` and still needs size inference.
    pub fn is_incomplete_array(self) -> bool {
        self.array_len == Some(0)
    }

    /// Returns true when this type can currently be lowered by the backend.
    pub fn is_supported_codegen_scalar(self) -> bool {
        if self.is_array() {
            return false;
        }
        if self.is_pointer() {
            return self.element_type().is_supported_pointer_target();
        }
        if self.has_struct_base() {
            return false;
        }
        if self.address_space != AddressSpace::Data {
            return false;
        }
        matches!(
            self.scalar,
            ScalarType::Void | ScalarType::I8 | ScalarType::U8 | ScalarType::I16 | ScalarType::U16
        )
    }

    /// Returns true when this type is an integer scalar value.
    pub fn is_integer(self) -> bool {
        !self.is_void() && !self.is_pointer() && !self.is_array() && !self.has_struct_base()
    }

    /// Returns true when this type is a scalar value that fits in registers or temps.
    pub fn is_scalar_value(self) -> bool {
        self.is_integer() || self.is_pointer()
    }

    /// Returns true when the scalar uses signed arithmetic semantics.
    pub fn is_signed(self) -> bool {
        self.is_integer() && matches!(self.scalar, ScalarType::I8 | ScalarType::I16)
    }

    /// Returns true when the scalar uses unsigned arithmetic semantics.
    pub fn is_unsigned(self) -> bool {
        self.is_integer() && matches!(self.scalar, ScalarType::U8 | ScalarType::U16)
    }

    /// Returns true when two pointer types can participate in the constrained Phase 3 model.
    pub fn same_pointer_target(self, other: Self) -> bool {
        self.is_pointer() && other.is_pointer() && self.element_type() == other.element_type()
    }

    /// Returns true when the type can be the target of a Phase 3 data pointer.
    pub fn is_supported_pointer_target(self) -> bool {
        self.array_len.is_none() && self.has_size() && self.address_space == AddressSpace::Data
    }

    /// Returns true when the type can live in a scalar value position in Phase 3.
    pub fn is_supported_value_type(self) -> bool {
        self.is_integer()
            || (self.is_pointer()
                && self.element_type().is_supported_pointer_target())
    }

    /// Returns true when the type can be declared as an addressable object in Phase 3.
    pub fn is_supported_object_type(self) -> bool {
        self.is_supported_value_type()
            || self.is_struct()
            || (self.is_array() && self.element_type().is_supported_pointer_target())
    }

    /// Returns the integer bit width associated with this type.
    pub fn bit_width(self) -> usize {
        self.byte_width() * 8
    }

    /// Returns the byte width needed to store this type in data memory.
    pub fn byte_width(self) -> usize {
        if let Some(len) = self.array_len {
            return self.element_type().byte_width() * len;
        }
        if self.is_pointer() {
            return 2;
        }
        if self.struct_id.is_some() {
            return self.struct_size;
        }
        match self.scalar {
            ScalarType::Void => 0,
            ScalarType::I8 | ScalarType::U8 => 1,
            ScalarType::I16 | ScalarType::U16 => 2,
        }
    }

    /// Returns the truncation mask that matches this type's value width.
    pub fn mask(self) -> i64 {
        match self.bit_width() {
            0 => 0,
            8 => 0x00FF,
            16 => 0xFFFF,
            _ => unreachable!("mask is only defined for scalar and pointer values"),
        }
    }

    /// Returns true when the type has a complete object size in the current phase.
    pub fn has_size(self) -> bool {
        !self.is_void()
            && !self.is_incomplete_array()
            && (self.struct_id.is_none() || self.struct_size > 0)
    }

    /// Returns the qualifiers that apply to the current object itself.
    pub fn object_qualifiers(self) -> Qualifiers {
        if self.is_pointer() {
            self.pointer_qualifiers[self.pointer_depth as usize - 1]
        } else {
            self.qualifiers
        }
    }

    /// Returns true when the current object itself is const-qualified.
    pub fn object_is_const(self) -> bool {
        self.object_qualifiers().is_const
    }

    /// Returns a copy with the current object's qualifiers replaced.
    pub fn with_object_qualifiers(mut self, qualifiers: Qualifiers) -> Self {
        if self.is_pointer() {
            let index = self.pointer_depth as usize - 1;
            self.pointer_qualifiers[index] = qualifiers;
        } else {
            self.qualifiers = qualifiers;
        }
        self
    }

    /// Returns one otherwise-identical type with top-level object qualifiers stripped.
    pub fn without_object_qualifiers(self) -> Self {
        self.with_object_qualifiers(Qualifiers::default())
    }

    /// Returns the qualifiers attached to one specific pointer level.
    pub const fn pointer_level_qualifiers(self, level: usize) -> Qualifiers {
        self.pointer_qualifiers[level]
    }

    /// Returns true when both types have the same underlying pointer shape if qualifiers are ignored.
    pub fn same_pointer_shape(self, other: Self) -> bool {
        self.is_pointer() && other.is_pointer() && self.unqualified() == other.unqualified()
    }

    /// Returns true when the declared object lives in program memory.
    pub fn is_rom(self) -> bool {
        self.address_space == AddressSpace::Rom
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
        if self.address_space == AddressSpace::Rom {
            formatter.write_str("__rom ")?;
        }
        let rendered_struct = if let Some(struct_id) = self.struct_id {
            format!("struct#{struct_id}")
        } else {
            match self.scalar {
                ScalarType::Void => "void".to_string(),
                ScalarType::I8 => "char".to_string(),
                ScalarType::U8 => "unsigned char".to_string(),
                ScalarType::I16 => "int".to_string(),
                ScalarType::U16 => "unsigned int".to_string(),
            }
        };
        formatter.write_str(&rendered_struct)?;
        for level in 0..self.pointer_depth {
            formatter.write_str("*")?;
            let qualifiers = self.pointer_qualifiers[level as usize];
            if qualifiers.is_const {
                formatter.write_str(" const")?;
            }
            if qualifiers.is_volatile {
                formatter.write_str(" volatile")?;
            }
        }
        if let Some(len) = self.array_len {
            if len == 0 {
                formatter.write_str("[]")?;
            } else {
                write!(formatter, "[{len}]")?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{AddressSpace, Qualifiers, ScalarType, Type};

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

    #[test]
    /// Verifies pointer and array helpers encode the constrained Phase 3 object model.
    fn phase_three_pointer_and_array_helpers_work() {
        let ptr = Type::new(ScalarType::U8).pointer_to();
        let array = Type::new(ScalarType::I16).array_of(4);

        assert!(ptr.is_pointer());
        assert_eq!(ptr.byte_width(), 2);
        assert_eq!(ptr.element_type(), Type::new(ScalarType::U8));

        assert!(array.is_array());
        assert_eq!(array.byte_width(), 8);
        assert_eq!(array.decay(), Type::new(ScalarType::I16).pointer_to());
    }

    #[test]
    /// Verifies pointer qualifiers distinguish top-level const pointers from pointers-to-const.
    fn phase_twelve_pointer_qualifiers_track_object_vs_pointee_const() {
        let ptr_to_const = Type::new(ScalarType::U8)
            .with_qualifiers(Qualifiers {
                is_const: true,
                is_volatile: false,
            })
            .pointer_to();
        let const_ptr = Type::new(ScalarType::U8).pointer_to_with_qualifiers(Qualifiers {
            is_const: true,
            is_volatile: false,
        });

        assert!(!ptr_to_const.object_is_const());
        assert!(ptr_to_const.element_type().qualifiers.is_const);
        assert!(const_ptr.object_is_const());
        assert!(!const_ptr.element_type().qualifiers.is_const);
    }

    #[test]
    /// Verifies ROM address-space metadata survives array wrapping and display rendering.
    fn phase_thirteen_rom_types_render_and_preserve_address_space() {
        let rom = Type::new(ScalarType::U8)
            .with_qualifiers(Qualifiers {
                is_const: true,
                is_volatile: false,
            })
            .with_address_space(AddressSpace::Rom)
            .array_of(4);

        assert!(rom.is_rom());
        assert!(rom.element_type().is_rom());
        assert_eq!(format!("{rom}"), "const __rom unsigned char[4]");
    }
}
// SPDX-License-Identifier: GPL-3.0-or-later
