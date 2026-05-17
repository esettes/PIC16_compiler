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
- Phase 28 dynamic casts support 16-bit integers and fixed-point types through the Q16.16 bridge
- Phase 29 adds dynamic 32-bit integer casts and dynamic comparisons
- Phase 30 adds ROM-backed float tables; those tables do not add new float arithmetic helpers

Resource note: generic float helpers are large. Codegen includes inline fast paths for common finite `* 2.0f` and `/ 2.0f` to avoid pulling generic helpers when possible. ROM float reads are cheap, but using the read result in helper-backed arithmetic/comparison can still pull large helpers.
## Phase 33 Cost Reporting

Phase 33 records float helpers in the centralized runtime helper catalog. `--size`, `--memory-report`, `.map`, and `.lst` show float helper category, actual emitted words, stack frame size, required-by text, and target constraints.

Large float helpers can exceed small targets. Use `--runtime-profile small --memory-report` before hardware builds when float is enabled.
