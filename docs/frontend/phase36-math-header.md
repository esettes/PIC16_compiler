<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Phase 36 Math Header

Phase 36 adds `include/math.h` as a small firmware-oriented header, not a full ISO C math library.

Supported prototypes:

```c
float fabsf(float x);
float truncf(float x);
float floorf(float x);
float ceilf(float x);
float roundf(float x);
```

The public header uses `GPL-3.0-or-later WITH GCC-exception-3.1` because it is intended for compiled firmware inputs.

Frontend behavior:

- supported calls require one `float` argument after normal argument coercion
- finite constant calls fold during semantic analysis
- `roundf` uses half-away-from-zero behavior
- `fabsf` may appear in an ISR because backend lowers it inline
- `truncf`, `floorf`, `ceilf`, and `roundf` are rejected in ISRs because they require runtime helpers

Unsupported:

- `double` math names such as `fabs`, `floor`, and `round`
- `sqrtf`, trigonometry, exponentials, logarithms, and `powf`
- NaN/Inf-specific behavior, `errno`, and fenv flags
