# Phase 29 Float Compare Helper

Phase 29 adds shared dynamic finite-float comparison helper:

```text
__rt_f32_cmp
```

ABI:

- argument 0: 32-bit raw `float`
- argument 1: 32-bit raw `float`
- return: `W`

Return values:

- `0xFF` when `a < b`
- `0x00` when `a == b`
- `0x01` when `a > b`

Supported lowering:

- `==`
- `!=`
- `<`
- `<=`
- `>`
- `>=`
- assignment to `unsigned char`
- `if`, `while`, and `for` conditions

Policy:

- finite values only
- `+0.0f` and `-0.0f` both compare as zero
- NaN/Inf behavior is unsupported
- helper-backed comparisons are rejected inside ISRs

Implementation note: Phase 42 compacts this helper. It now uses raw finite f32 sign/exponent/mantissa ordering instead of converting both operands through Q16.16. Values outside the finite model are still not full IEEE comparisons.

Phase 30 ROM float reads produce ordinary raw f32 values. Comparing a ROM-read value is supported after the value is materialized like any other float; helper cost and ISR restrictions still apply.
## Phase 33 Catalog Entry

`__rt_f32_cmp` is categorized as a float helper in Phase 33 reports. Its emitted word count, frame bytes, page placement, and dependency graph entry appear in `--memory-report` and `.map`.

Phase 42 keeps `__rt_f32_cmp` standalone but makes it substantially smaller. `fminf` and `fmaxf` now reuse this helper directly at the call site and do not emit separate min/max helpers.
