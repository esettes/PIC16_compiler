<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Phase 36 Float Math Helpers

Phase 36 adds finite-only float math helpers:

```text
__rt_f32_fabs
__rt_f32_trunc
__rt_f32_floor
__rt_f32_ceil
__rt_f32_round
```

Behavior:

- `fabsf` clears the f32 sign bit
- `truncf` converts f32 to signed 32-bit integer and back, truncating toward zero
- `floorf` computes `t = truncf(x)` and returns `t - 1.0f` when `x < t`
- `ceilf` computes `t = truncf(x)` and returns `t + 1.0f` when `x > t`
- `roundf` returns half away from zero using `floorf(x + 0.5f)` or `ceilf(x - 0.5f)`

Runtime integration:

- helpers are in the runtime catalog with category `math`
- dependency graph tracks conversion, comparison, and arithmetic helpers used by composed math helpers
- `--size`, `--memory-report`, `.map`, and `.lst` show math helper contribution
- helpers are pruned when unused

Limitations:

- finite values only
- no NaN/Inf semantics
- no `double`
- no `sqrtf` or trigonometric functions
- helper-backed math is rejected inside ISRs except inline `fabsf`
