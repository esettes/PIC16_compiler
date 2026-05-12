# Phase 27 Float Codegen

Backend behavior:

- `float` uses the existing 4-byte scalar storage path
- 32-bit returns use `W`, `return_high`, `return_upper0`, and `return_upper1`
- unary minus toggles the sign bit inline
- finite `* 2.0f` and `/ 2.0f` adjust exponent bits inline
- other dynamic float arithmetic emits `__rt_f32_*` helpers
- emitted helpers are classified as `float helper` in memory reports

Deferred:

- ROM float tables
- dynamic float/integer conversion helpers
- full IEEE NaN/Inf/subnormal handling
- math library functions

Resource fitting remains authoritative. If float helpers exceed a target, the compiler emits program-memory or stack diagnostics instead of writing invalid HEX.
