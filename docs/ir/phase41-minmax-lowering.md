<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Phase 41 Min/Max Lowering

`fminf` and `fmaxf` remain ordinary direct call IR nodes after semantic analysis unless both arguments fold as constants.

Backend lowering:

```text
fminf -> __rt_f32_min
fmaxf -> __rt_f32_max
```

Both helpers use the Stack-first ABI:

- argument 0: 4-byte raw f32
- argument 1: 4-byte raw f32
- return: 4-byte raw f32 via the existing 32-bit return convention

The helpers depend on:

```text
__rt_f32_cmp
```

`--math-profile` does not alter min/max semantics. `--runtime-profile` can still affect global helper layout/reporting, but Phase 41 min/max helper behavior is the same across profiles.
