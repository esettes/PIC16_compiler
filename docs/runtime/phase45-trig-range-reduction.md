<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Phase 45 Trig Range Reduction

Phase 45 does not implement full range reduction. Phase 46 extends the same bounded strategy with more scoped aliases.

Implemented hardening:

- validated range remains `[-2pi,+2pi]`
- additional exact point aliases cover common moderate multiples:
  - `3pi`
  - `-3pi`
  - `4pi`
  - `-4pi`
- Phase 46 extends aliases through:
  - `5pi`
  - `-5pi`
  - `6pi`
  - `-6pi`
  - `8pi`
  - `-8pi`
- fallback remains deterministic and coarse outside documented points

Reason:

Full range reduction for arbitrary finite f32 inputs would require more runtime code and likely more helper dependencies. Phase 45/46 keep cost bounded while improving common firmware cases.

Policy:

```text
Input: radians
Runtime: finite-only
NaN/Inf: unsupported
errno/fenv: unsupported
IEEE/libm: not claimed
```

For exact behavior outside documented points, use constants folded by compiler, ROM calibration tables, or fixed-point lookup logic.
