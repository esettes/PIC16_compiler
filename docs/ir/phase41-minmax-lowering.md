<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Phase 41 Min/Max Lowering

`fminf` and `fmaxf` remain ordinary direct call IR nodes after semantic analysis unless both arguments fold as constants.

Backend lowering after Phase 42:

```text
fminf -> __rt_f32_cmp + local select
fmaxf -> __rt_f32_cmp + local select
```

The compare helper uses the Stack-first ABI:

- argument 0: 4-byte raw f32
- argument 1: 4-byte raw f32
- return: one compare byte in `W` (`0xff`, `0`, or `1`)

No separate min/max helper body is emitted:

```text
__rt_f32_min / __rt_f32_max: pruned / not generated
```

`--math-profile` does not alter min/max semantics. `--runtime-profile` can still affect global helper layout/reporting, but Phase 41 min/max helper behavior is the same across profiles.
