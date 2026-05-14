# Phase 27 Float Codegen

Backend behavior:

- `float` uses the existing 4-byte scalar storage path
- 32-bit returns use `W`, `return_high`, `return_upper0`, and `return_upper1`
- unary minus toggles the sign bit inline
- finite `* 2.0f` and `/ 2.0f` adjust exponent bits inline
- other dynamic float arithmetic emits `__rt_f32_*` helpers
- emitted helpers are classified as `float helper` in memory reports

Deferred:

- `double`
- full IEEE NaN/Inf/subnormal handling
- math library functions

Later phases:

- Phase 28 adds 16-bit/fixed dynamic conversion helpers.
- Phase 29 adds dynamic comparisons and 32-bit integer conversion helpers.
- Phase 30 adds ROM float tables through RETLW-backed raw f32 bytes.

Resource fitting remains authoritative. If float helpers exceed a target, the compiler emits program-memory or stack diagnostics instead of writing invalid HEX.
