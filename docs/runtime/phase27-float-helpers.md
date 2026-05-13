# Phase 27 Float Helpers

Runtime helper labels:

- `__rt_f32_add`
- `__rt_f32_sub`
- `__rt_f32_mul`
- `__rt_f32_div`
- `__rt_q16_16_to_f32` (Phase 28 cast bridge)
- `__rt_f32_to_q16_16` (Phase 28 cast bridge)
- `__rt_f32_cmp` (Phase 29 comparisons)
- `__rt_i32_to_f32` / `__rt_u32_to_f32` (Phase 29 32-bit integer to float)
- `__rt_f32_to_i32` / `__rt_f32_to_u32` (Phase 29 float to 32-bit integer)

The implemented finite helper strategy stores public values as IEEE-754 f32 bits, then converts operands into an internal signed Q16.16 work format for helper arithmetic. Results convert back to f32 bits.

Behavior:

- dynamic division by zero follows the internal fixed helper policy and returns raw zero
- constant float division by zero is a diagnostic
- overflow/wrap follows the internal Q16.16 work representation, not full IEEE saturation/NaN/Inf behavior
- helper-backed float operations are rejected inside ISRs
- helper labels appear in `.map`, `.lst`, stack reports, and memory reports when emitted
- Phase 28 dynamic casts support 16-bit integers and fixed-point types; dynamic 32-bit integer casts and dynamic float comparisons remain rejected
- Phase 29 adds dynamic 32-bit integer casts and dynamic comparisons

Resource note: generic float helpers are large. Codegen includes inline fast paths for common finite `* 2.0f` and `/ 2.0f` to avoid pulling generic helpers when possible.
