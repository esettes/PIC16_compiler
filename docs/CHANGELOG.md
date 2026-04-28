# Changelog

## Phase 8 - Type System and Initializers

- Added: `typedef` support for type aliases.
- Added: `enum` declarations and constants (16-bit representation).
- Added: named `struct` declarations with packed field layout.
- Added: positional aggregate initializers for arrays and structs (globals flattened to bytes, locals lowered to assignment sequences).
- Added: explicit cast syntax and semantic validation.
- Added: member access operators `.` and `->` lowered to pointer+offset+deref.

Restrictions and design decisions:
- No nested aggregates or designated initializers in this phase.
- Structs are packed with no padding.
- Phase 4 ABI and Phase 6 ISR model remain unchanged.
