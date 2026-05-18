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

Unsupported behavior:

- no `sqrt`, `sqrtl`, `powf`, or trigonometric functions
- no NaN/Inf, `errno`, or fenv behavior
- helper-backed `sqrtf` is rejected inside ISRs
