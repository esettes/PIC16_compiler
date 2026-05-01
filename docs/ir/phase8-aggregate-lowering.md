<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Phase 8: IR Aggregate Lowering

In Phase 8 the frontend accepts positional aggregate initializers for one-dimensional arrays of supported scalar targets and for flat named structs with non-aggregate fields. The semantic pass flattens these initializers into a per-slot sequence of stores for local variables, and into a pre-materialized byte array for global variables.

Key points:
- Global aggregate initializer elements must be constant-evaluable at compile time. They are converted into a contiguous little-endian byte sequence and written through backend startup writes.
- Local aggregate initializers expand into per-element or per-field stores in declaration order.
- Missing array elements and missing struct fields are zero-filled up to the declared object size.
- Too many array or struct initializer elements are diagnosed before IR emission.
- Whole-struct assignment, nested aggregates, and designated initializers are not lowered in this phase; they are rejected in the frontend.
<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
