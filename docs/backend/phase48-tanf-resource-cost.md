<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Phase 48 `tanf` Resource Cost

Phase 48 keeps tangent resource cost bounded by reusing the shared trig core.

Expected report shape:

```text
Math profile: balanced
Math helpers:
  __rt_f32_tan variant=wrapper
  __rt_f32_sincos_core variant=shared_core_balanced
Runtime Helper Dependency Graph:
  __rt_f32_tan -> __rt_f32_sincos_core
```

Pruning:

- unused `<math.h>` emits no trig helpers/tables
- folded `tanf` emits no trig helpers/tables
- tan-only emits `__rt_f32_tan`, `__rt_f32_sincos_core`, and the selected ROM table
- tan-only does not emit `__rt_f32_sin` or `__rt_f32_cos`
- mixed sin/cos/tan programs reuse one shared core and one table

Dynamic precise-profile tangent is deferred and fails with a diagnostic before report generation. PIC16F877A is the expected target for dynamic tangent examples; smaller targets can fail resource fitting.
