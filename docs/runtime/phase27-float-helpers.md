# Phase 27 Float Helpers

Runtime helper labels:

- `__rt_f32_add`
- `__rt_f32_sub`
- `__rt_f32_mul`
- `__rt_f32_div`

The implemented finite helper strategy stores public values as IEEE-754 f32 bits, then converts operands into an internal signed Q16.16 work format for helper arithmetic. Results convert back to f32 bits.

Behavior:

- dynamic division by zero follows the internal fixed helper policy and returns raw zero
- constant float division by zero is a diagnostic
- overflow/wrap follows the internal Q16.16 work representation, not full IEEE saturation/NaN/Inf behavior
- helper-backed float operations are rejected inside ISRs
- helper labels appear in `.map`, `.lst`, stack reports, and memory reports when emitted

Resource note: generic float helpers are large. Codegen includes inline fast paths for common finite `* 2.0f` and `/ 2.0f` to avoid pulling generic helpers when possible.
