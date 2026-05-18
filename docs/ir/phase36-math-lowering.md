<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Phase 36 Math Lowering

Math calls remain ordinary direct call IR nodes. This keeps the frontend simple and lets the backend map known symbols to runtime helpers:

```text
fabsf  -> __rt_f32_fabs
truncf -> __rt_f32_trunc
floorf -> __rt_f32_floor
ceilf  -> __rt_f32_ceil
roundf -> __rt_f32_round
sqrtf -> __rt_f32_sqrt
```

Constant calls fold before IR lowering when the argument is a finite raw f32 constant.

Dynamic calls use the existing Stack-first ABI:

- argument: one 4-byte raw f32 value
- return: existing 32-bit convention (`W`, `return_high`, `return_upper0`, `return_upper1`)
- helper labels are emitted only when the call survives folding and pruning

No new C type, IR scalar, or linker model is introduced.
