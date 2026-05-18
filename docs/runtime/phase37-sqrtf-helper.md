<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Phase 37 `sqrtf` Helper

Runtime helper:

```text
__rt_f32_sqrt
```

Algorithm:

- convert finite f32 input to signed Q16.16 inside the helper
- return raw `0.0f` when the Q16.16 input is zero or negative
- search the Q16.16 interval `[0, max(x, 1.0)]` with 20 bisection iterations
- use an internal local unsigned Q16.16 multiplication block instead of emitting external conversion/multiply helpers

Runtime integration:

- category: `math`
- dependencies: none; Q16.16 conversion/division logic is internal to keep program size bounded
- Stack-first ABI: one 4-byte argument, one 4-byte return value
- appears in `.map`, `.lst`, `--size`, `--memory-report`, and stack reports when used
- pruned when unused or when calls fold at compile time

Policy:

- finite-only
- negative input returns `0.0f`
- no NaN/Inf, `errno`, fenv, or signed-zero promise
- `small` and `balanced` currently share the same compact `sqrtf` helper body
