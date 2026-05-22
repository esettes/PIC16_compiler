<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Phase 37 `sqrtf`

Phase 37 adds one finite math function:

```c
#include <math.h>

float y = sqrtf(x);
```

Supported behavior:

- one `float` argument
- one `float` result
- finite values only
- `sqrtf(0.0f)`, `sqrtf(1.0f)`, `sqrtf(4.0f)`, `sqrtf(9.0f)`, `sqrtf(2.25f)`, and `sqrtf(0.25f)` are simulator-tested
- negative finite input returns `0.0f`
- other positive finite inputs use a compact approximation fallback
- `sqrtf` is not correctly-rounded IEEE-754 `sqrt`

Unsupported behavior:

- no `sqrt`, `sqrtl`, `powf`, or trigonometric functions
- no NaN/Inf, `errno`, or fenv behavior
- helper-backed `sqrtf` is rejected inside ISRs
- use fixed-point or validated calibration tables if exact behavior matters

Phase 38 adds `--math-profile compact|balanced|precise` without changing this source syntax. `compact` uses the behavior above, `balanced` currently aliases it, and `precise` currently diagnoses dynamic `sqrtf` on PIC16 instead of silently using the compact helper.
