<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Phase 41 Min/Max Helpers

Phase 41 introduced finite min/max. Phase 42 compacts the runtime path and no longer emits separate min/max helper bodies:

```text
fminf/fmaxf -> __rt_f32_cmp + local select
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

The implementation calls `__rt_f32_cmp` from the user call site, stores the comparison result in compiler scratch RAM, and copies one original operand to the destination. This removes the former `__rt_f32_min` and `__rt_f32_max` wrappers.

Policy:

- finite-only
- no NaN
- no Inf
- no `errno`
- no fenv
- no ISO/IEEE signed-zero guarantee

Resource behavior:

- emitted helper category: `float` (`__rt_f32_cmp`)
- no emitted `__rt_f32_min` / `__rt_f32_max`
- visible in `.map`, `.lst`, `--size`, `--memory-report`, and stack reports
- pruned when unused or when calls fold at compile time
- rejected inside ISRs because dynamic calls require helper execution
