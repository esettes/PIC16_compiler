<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Phase 46 Trig Moderate Range Reduction

Phase 46 keeps finite table-driven `sinf` / `cosf` and extends moderate range handling through scoped aliases.

Policy:

```text
input unit: radians
runtime model: finite-only
NaN/Inf: unsupported
errno/fenv: unsupported
IEEE/libm: not claimed
```

Implemented behavior:

- base validated point grid remains in `[-2pi,+2pi]`
- common multiples `±3pi`, `±4pi`, `±5pi`, `±6pi`, and `±8pi` are matched deterministically
- compact tolerance remains `<= 0.10`
- balanced tolerance remains `<= 0.05`
- precise dynamic trig remains deferred

This is not continuous argument reduction. The helper matches documented f32 literal spellings used by the simulator harness. Other finite values outside the documented grid use the coarse finite fallback.

Reason:

Full argument reduction would require larger runtime code and more helper dependencies. Phase 46 improves common firmware angles while preserving bounded PIC16 resource cost.

Phase 47 keeps this behavior and reduces core size by matching positive magnitudes once, then applying sine sign for negative inputs.
