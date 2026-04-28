# Phase 8: Backend Struct Layout

Structs in Phase 8 use a packed layout: fields are placed back-to-back with no padding. Field offsets are computed during parsing and validated by the semantic pass.

Backend responsibilities:
- Treat struct fields as byte offsets from the struct base address.
- The code generator lowers `.` access to `AddrOf(base) + offset` followed by the appropriate load/store for the field width.
- Global struct initializers are emitted as contiguous bytes written at startup.

Limitations:
- No alignment/padding rules are implemented in this phase.
- Nested structs and array fields inside structs are rejected in the frontend.
