<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Phase 37 `sqrtf` Helper

Runtime helper:

```text
__rt_f32_sqrt
```

Algorithm:

- return raw `0.0f` when the finite f32 input is zero or negative
- initialize the f32 guess from the input value
- run eight deterministic Newton iterations: `guess = (guess + x / guess) / 2`
- use the existing finite f32 add/divide helpers for the iteration

Runtime integration:

- category: `math`
- dependencies: `__rt_f32_div`, `__rt_f32_add`
- Stack-first ABI: one 4-byte argument, one 4-byte return value
- appears in `.map`, `.lst`, `--size`, `--memory-report`, and stack reports when used
- pruned when unused or when calls fold at compile time

Policy:

- finite-only
- negative input returns `0.0f`
- no NaN/Inf, `errno`, fenv, or signed-zero promise
- `small` and `balanced` currently share the same `sqrtf` helper body; dependencies still follow the selected runtime profile
