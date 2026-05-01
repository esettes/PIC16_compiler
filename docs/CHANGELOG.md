<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Changelog

## v0.1.0 - 2026-04-28

Release v0.1.0 corresponds to the Phase 8 milestone; see `docs/releases/v0.1.0.md` for full notes.

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
<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
