<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Phase 41 `fminf` / `fmaxf`

Phase 41 extends the minimal finite math subset:

```c
#include <math.h>

float low = fminf(a, b);
float high = fmaxf(a, b);
```

Supported:

- exactly two `float` arguments
- one `float` result
- finite values only
- constant folding when both arguments are finite constants
- dynamic calls outside ISRs

Behavior:

```text
fminf(1.0f, 2.0f)   -> 1.0f
fminf(-3.0f, -2.0f) -> -3.0f
fmaxf(1.0f, 2.0f)   -> 2.0f
fmaxf(-3.0f, -2.0f) -> -2.0f
```

Unsupported:

- `fmin`, `fmax`, `fminl`, `fmaxl`
- NaN/Inf handling
- ISO/IEEE signed-zero selection guarantees
- helper-backed dynamic calls inside ISRs
