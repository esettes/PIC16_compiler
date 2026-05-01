<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Phase 8: Backend Struct Layout

Structs in Phase 8 use a packed layout: fields are placed back-to-back with no padding. The frontend computes field offsets in declaration order, and the backend consumes that metadata directly.

Backend responsibilities:
- Treat struct fields as byte offsets from the struct base address.
- The code generator lowers `.` access to `AddrOf(base) + offset` followed by the appropriate load/store for the field width.
- The code generator lowers `->` access the same way after dereferencing the base pointer.
- Global struct initializers arrive as contiguous bytes and are written during startup emission.

Limitations:
- No alignment/padding rules are implemented in this phase.
- Nested structs and array fields inside structs are rejected in the frontend.
- Whole-struct load/store/copy operations are not implemented; field-wise access is required.
<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
