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
float sqrtf(float x);
float fminf(float a, float b);
float fmaxf(float a, float b);
```

The public header uses `GPL-3.0-or-later WITH GCC-exception-3.1` because it is intended for compiled firmware inputs.

Frontend behavior:

- supported unary calls require one `float` argument and Phase 41 min/max calls require two `float` arguments
- finite constant calls fold during semantic analysis
- `roundf` uses half-away-from-zero behavior
- `sqrtf` returns `0.0f` for negative finite inputs
- `fminf` / `fmaxf` select the lower/higher finite operand and do not model NaN/Inf
- `fabsf` may appear in an ISR because backend lowers it inline
- `truncf`, `floorf`, `ceilf`, `roundf`, `sqrtf`, `fminf`, and `fmaxf` are rejected in ISRs because they require runtime helpers

Unsupported:

- `double` math names such as `fabs`, `floor`, `round`, `fmin`, and `fmax`
- trigonometry, exponentials, logarithms, and `powf`
- NaN/Inf-specific behavior, `errno`, and fenv flags
