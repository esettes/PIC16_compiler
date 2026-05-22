<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Math Subset

Use the Phase 36 math subset by including:

```c
#include <math.h>
```

Supported:

```c
float a = fabsf(x);
float b = truncf(x);
float c = floorf(x);
float d = ceilf(x);
float e = roundf(x);
float f = sqrtf(x);
```

`roundf` rounds half away from zero:

```text
roundf(1.5f)  ->  2.0f
roundf(-1.5f) -> -2.0f
```

`sqrtf` is finite-only. Negative inputs return `0.0f` because this compiler does not model NaN or Inf:

```text
sqrtf(2.25f) -> 1.5f
sqrtf(-1.0f) -> 0.0f
```

Dynamic `sqrtf` is deliberately compact. It returns exact documented results for the validated values above and uses an approximation fallback for other positive finite values. It is not a correctly-rounded IEEE-754 square root. Use fixed-point or validated calibration tables if exact behavior matters.

Recommended workflow:

```bash
picc --target pic16f877a -I include --size --memory-report -o build/math.hex examples/pic16f877a/math_round.c
```

Notes:

- PIC16F877A is the safer default for math-heavy examples
- `fabsf` can be inline-safe in ISRs
- `truncf`, `floorf`, `ceilf`, `roundf`, and `sqrtf` pull helpers and are rejected in ISRs
- unused `#include <math.h>` does not emit math helpers
- unsupported names such as `sqrt`, `sin`, or `fabs` are intentionally not declared
