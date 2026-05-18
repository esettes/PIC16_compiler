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
```

`roundf` rounds half away from zero:

```text
roundf(1.5f)  ->  2.0f
roundf(-1.5f) -> -2.0f
```

Recommended workflow:

```bash
picc --target pic16f877a -I include --size --memory-report -o build/math.hex examples/pic16f877a/math_round.c
```

Notes:

- PIC16F877A is the safer default for math-heavy examples
- `fabsf` can be inline-safe in ISRs
- `truncf`, `floorf`, `ceilf`, and `roundf` pull helpers and are rejected in ISRs
- unused `#include <math.h>` does not emit math helpers
- unsupported names such as `sqrtf`, `sin`, or `fabs` are intentionally not declared
