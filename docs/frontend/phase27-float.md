# Phase 27 Float Frontend

Phase 27 adds `float` as a distinct scalar type.

Model:

- `sizeof(float) == 4`
- storage is little-endian IEEE-754 single-precision bits
- supported literals: `1.5`, `1.5f`, `0.125f`
- supported values: finite normal values and zero for tested runtime paths
- unsupported/deferred: `double`, `nan`, `inf`, float switch expressions, float bitfields, and float array bounds

Conversions are conservative. Constant casts between integer/fixed/float are folded. Phase 28 adds runtime casts for 16-bit integers and fixed-point values through Q16.16 bridge helpers. Phase 29 adds dynamic 32-bit integer casts and dynamic float comparisons. Phase 30 adds file-scope `const __rom float[]` tables with direct indexing. Dynamic mixed float/integer arithmetic is still rejected; cast explicitly and keep runtime-heavy conversion work out of ISR code.

Runtime helpers are finite-only and resource-heavy. They are meant for small embedded scaling cases, not full IEEE-754 conformance.
