<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Phase 41 Min/Max Helpers

Runtime helpers:

```text
__rt_f32_min
__rt_f32_max
```

Algorithm:

```text
fminf(a, b):
  if a <= b: return a
  return b

fmaxf(a, b):
  if a >= b: return a
  return b
```

The implementation calls `__rt_f32_cmp`, stores the comparison result, and copies one original operand to the 32-bit return slot.

Policy:

- finite-only
- no NaN
- no Inf
- no `errno`
- no fenv
- no ISO/IEEE signed-zero guarantee

Resource behavior:

- category: `math`
- dependency: `__rt_f32_cmp`
- visible in `.map`, `.lst`, `--size`, `--memory-report`, and stack reports
- pruned when unused or when calls fold at compile time
- rejected inside ISRs because dynamic calls require helper execution
