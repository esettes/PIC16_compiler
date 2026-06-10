<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Phase 36 Float Math Helpers

Phase 36 adds finite-only float math helpers:

```text
__rt_f32_fabs
__rt_f32_trunc
__rt_f32_floor
__rt_f32_ceil
__rt_f32_round
__rt_f32_sqrt
__rt_f32_sin
__rt_f32_cos
__rt_f32_sincos_core
```

Behavior:

- `fabsf` clears the f32 sign bit
- `truncf` converts f32 to signed 32-bit integer and back, truncating toward zero
- `floorf` converts to Q16.16 and arithmetically shifts away the fractional bits
- `ceilf` reuses the floor helper through the identity `ceilf(x) = -floorf(-x)`
- `roundf` converts to absolute Q16.16, adds raw `0.5`, truncates to an integer, reapplies the sign, and converts back to f32
- Phase 37 `sqrtf` returns exact validated finite cases, uses a compact bit-level approximation for other positive values, and returns `0.0f` for negative inputs
- Phase 41 `fminf` / `fmaxf` return one original operand; Phase 42 lowers them to `__rt_f32_cmp` plus local select instead of separate min/max helpers
- Phase 44 `sinf` / `cosf` use small wrappers around one finite table-backed shared core in radians; Phase 49 `tanf` uses a separate tangent core; compact and balanced helpers are approximate and not libm replacements

Runtime integration:

- helpers are in the runtime catalog with category `math`
- dependency graph tracks conversion, comparison, and arithmetic helpers used by composed math helpers
- `--size`, `--memory-report`, `.map`, and `.lst` show math helper contribution
- helpers are pruned when unused

Limitations:

- finite values only
- no NaN/Inf semantics
- no `double`
- no `atanf`, exp/log/pow, or full ISO C trigonometry
- helper-backed math is rejected inside ISRs except inline `fabsf`
- no NaN/Inf or ISO signed-zero min/max semantics
