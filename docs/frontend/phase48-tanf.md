<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Phase 48 `tanf`

`include/math.h` declares:

```c
float tanf(float x);
```

Rules:

- one `float` argument
- returns `float`
- input unit is radians
- finite-only runtime model
- `tan` / `double` forms are unsupported and diagnose
- helper-backed dynamic calls are rejected inside ISRs
- finite constant calls fold before backend lowering

No NaN, Inf, errno, fenv, `double`, `atanf`, `atan2f`, or full ISO C `math.h` behavior is provided.

Dynamic `--math-profile precise` tangent is deferred and diagnoses. It never silently falls back to compact/balanced runtime behavior.
