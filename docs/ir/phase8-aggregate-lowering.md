# Phase 8: IR Aggregate Lowering

In Phase 8 the frontend accepts positional aggregate initializers for arrays and structs. The semantic pass flattens these initializers into a per-element sequence of assignments when used for local variables, and into a pre-materialized byte array for global variables.

Key points:
- Global aggregate initializers must be constant-evaluable at compile time. They are converted into a contiguous byte sequence matching the target endianness and written into ROM/ROM-like startup via backend "startup writes".
- Local aggregate initializers expand to a compound statement with per-element stores and respect the declared element order.
- Nested aggregates and designated initializers are not allowed in this phase.
- Arrays initialized with fewer elements than their declared size are zero-filled up to the array length.
