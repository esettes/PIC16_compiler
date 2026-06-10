<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Phase 48/49 `tanf` Resource Cost

Phase 48 added finite `tanf`; Phase 49 reduces tan-only cost by moving tangent-specific matching into an isolated core.

Expected report shape:

```text
Math profile: balanced
Math helpers:
  __rt_f32_tan variant=wrapper
  __rt_f32_tan_core variant=tan_core_balanced
Runtime Helper Dependency Graph:
  __rt_f32_tan -> __rt_f32_tan_core
```

Pruning:

- unused `<math.h>` emits no trig helpers/tables
- folded `tanf` emits no trig helpers/tables
- tan-only emits `__rt_f32_tan` and `__rt_f32_tan_core`
- tan-only does not emit `__rt_f32_sin` or `__rt_f32_cos`
- tan-only does not emit `__rt_f32_sincos_core`
- mixed sin/cos/tan programs emit `__rt_f32_sincos_core` for sine/cosine and `__rt_f32_tan_core` for tangent

Dynamic precise-profile tangent is deferred and fails with a diagnostic before report generation. PIC16F877A is the expected target for dynamic tangent examples; smaller targets can fail resource fitting.
