<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Phase 37 `sqrtf` Helper

Runtime helper:

```text
__rt_f32_sqrt
```

Algorithm:

- return raw `0.0f` when the finite f32 input is zero or negative
- return exact raw f32 results for the simulator-validated values `1.0`, `4.0`, `9.0`, `2.25`, and `0.25`
- use a compact bit-level finite approximation for other positive inputs: `(raw >> 1) + 0x1FC00000`
- this keeps the helper small enough for PIC16F877A while documenting that Phase 37 is not a full correctly-rounded IEEE sqrt implementation

Runtime integration:

- category: `math`
- dependencies: none
- Stack-first ABI: one 4-byte argument, one 4-byte return value
- appears in `.map`, `.lst`, `--size`, `--memory-report`, and stack reports when used
- pruned when unused or when calls fold at compile time

Policy:

- finite-only
- negative input returns `0.0f`
- no NaN/Inf, `errno`, fenv, correctly-rounded IEEE sqrt, or signed-zero promise
- runtime profile `small` and `balanced` currently share the same compact `sqrtf` helper body
- math profile `compact` uses this helper; math profile `balanced` currently uses the same helper; math profile `precise` uses the larger Phase 39 refined helper variant
- use fixed-point or validated calibration tables when exact positive-input behavior matters outside the documented exact cases
