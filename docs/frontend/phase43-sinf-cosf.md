<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Phase 43 `sinf` / `cosf`

Phase 43 adds finite-only declarations:

```c
float sinf(float x);
float cosf(float x);
```

Rules:

- input is `float` radians
- exactly one argument
- integer/fixed inputs require explicit `(float)` casts
- `sin` and `cos` double names are unsupported and diagnose
- no NaN, Inf, errno, fenv, or full ISO C math behavior

Constant finite calls fold in semantic analysis with host `f32` behavior. Dynamic calls lower to finite runtime helpers and are rejected inside ISRs.

Dynamic validated points include `0`, `±pi/6`, `±pi/4`, `±pi/3`, `±pi/2`, `±2pi/3`, `±3pi/4`, `±5pi/6`, `±pi`, `±3pi/2`, and `±2pi` spellings used by the simulator tests. Phase 45 also validates deterministic `±3pi` and `±4pi` aliases. Fallback outside documented table points is coarse and finite.
