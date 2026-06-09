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
float g = fminf(a, b);
float h = fmaxf(a, b);
float i = sinf(x);
float j = cosf(x);
float k = tanf(x);
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

Phase 38 adds math profiles:

```bash
picc --math-profile compact ...
picc --math-profile balanced ...
picc --math-profile precise ...
```

`compact` uses the Phase 37 compact approximation. `balanced` is the default and currently behaves the same as `compact`. `precise` uses the larger `precise_table_refined` helper for dynamic `sqrtf`.

Phase 40 validates precise `sqrtf` against host `f32::sqrt` for positive inputs in `[0.25, 64.0]` with absolute tolerance `±0.03125`. The validated inputs are:

```text
0.25, 0.5, 1.0, 2.0, 3.0, 4.0, 5.0, 8.0, 9.0, 10.0, 16.0, 25.0, 36.0, 49.0, 64.0
```

Outside the refined table, dynamic precise `sqrtf` still falls back to the compact approximation. Constant `sqrtf` folding uses the compile-time finite result and does not emit `__rt_f32_sqrt`.

`fminf` and `fmaxf` are finite-only:

```text
fminf(1.0f, 2.0f)   -> 1.0f
fminf(-3.0f, -2.0f) -> -3.0f
fmaxf(1.0f, 2.0f)   -> 2.0f
fmaxf(-3.0f, -2.0f) -> -2.0f
```

They do not implement NaN/Inf behavior. For `+0.0f` and `-0.0f`, the selected result is implementation-defined within the finite-only model.

`sinf`, `cosf`, and `tanf` are finite-only Phase 43/48 approximations:

```text
sinf(0.0f)        -> 0.0f
cosf(0.0f)        -> 1.0f
sinf(1.5707963f) -> approximately 1.0f
cosf(3.1415927f) -> approximately -1.0f
tanf(0.7853982f) -> approximately 1.0f
tanf(1.5707963f) -> +32767.0f saturation
```

Inputs are radians. Dynamic compact and balanced helpers use one shared `__rt_f32_sincos_core`, internal ROM quarter-wave tables, and validated table points in `[-2π, +2π]`. Compact tolerance is `<= 0.10` for `sinf` / `cosf`; balanced tolerance is `<= 0.05` for simulator-validated `sinf` / `cosf` points. Phase 48 `tanf` uses the same core in TAN mode with non-pole tolerance `<= 0.20` for compact and `<= 0.10` for balanced. Pole-like odd `π/2` inputs return finite saturation `+32767.0f` or `-32767.0f`; no Inf is produced. Phase 46 validates deterministic common-multiple aliases through `±8π`. Phase 47 compacts those aliases by matching positive magnitudes once and applying sine/tangent signs afterward. Outside documented points, runtime fallback is coarse and finite; use ROM calibration tables or fixed-point if exact trig behavior matters.

`--math-profile precise` currently diagnoses dynamic `sinf` / `cosf` / `tanf` as deferred. It does not silently use compact behavior. Constant finite `sinf` / `cosf` / `tanf` calls fold at compile time with host `f32` behavior, with tangent results clamped to the finite saturation policy, and do not emit trig helpers.

Recommended workflow:

```bash
picc --target pic16f877a -I include --math-profile compact --size --memory-report -o build/math.hex examples/pic16f877a/math_round.c
```

Notes:

- PIC16F877A is the safer default for math-heavy examples
- `fabsf` can be inline-safe in ISRs
- `truncf`, `floorf`, `ceilf`, `roundf`, `sqrtf`, `fminf`, `fmaxf`, `sinf`, `cosf`, and `tanf` pull helpers and are rejected in ISRs
- unused `#include <math.h>` does not emit math helpers
- unsupported names such as `sqrt`, `sin`, `cos`, `tan`, or `fabs` are intentionally not declared
