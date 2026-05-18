<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Phase 37 `sqrtf` Helper

Runtime helper:

```text
__rt_f32_sqrt
```

Algorithm:

- convert finite f32 input to signed Q16.16
- return raw `0.0f` when the Q16.16 input is zero or negative
- choose an initial Q16.16 guess of `max(x, 1.0)`
- run eight deterministic Newton iterations: `guess = (guess + x / guess) / 2`
- convert the Q16.16 result back to f32

Runtime integration:

- category: `math`
- dependencies: `__rt_f32_to_q16_16`, `__rt_div_q16_16`, `__rt_q16_16_to_f32`
- Stack-first ABI: one 4-byte argument, one 4-byte return value
- appears in `.map`, `.lst`, `--size`, `--memory-report`, and stack reports when used
- pruned when unused or when calls fold at compile time

Policy:

- finite-only
- negative input returns `0.0f`
- no NaN/Inf, `errno`, fenv, or signed-zero promise
- `small` and `balanced` currently share the same `sqrtf` helper body; dependencies still follow the selected runtime profile
