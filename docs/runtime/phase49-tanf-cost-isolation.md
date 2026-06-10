<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Phase 49 `tanf` Cost Isolation

Phase 49 separates tangent runtime code from the sine/cosine shared core.

Runtime shape:

```text
sinf -> __rt_f32_sin -> __rt_f32_sincos_core
cosf -> __rt_f32_cos -> __rt_f32_sincos_core
tanf -> __rt_f32_tan -> __rt_f32_tan_core
```

This keeps tangent-specific finite point matches and pole saturation out of
`__rt_f32_sincos_core`. Sin/cos-only programs retain the Phase 47 compact core;
tan-only programs no longer emit sine/cosine wrappers or `__rt_f32_sincos_core`.

The tangent behavior is unchanged from Phase 48:

- radians input
- finite-only runtime policy
- no NaN, Inf, errno, or fenv
- no correctly-rounded libm claim
- non-pole compact tolerance `<= 0.20`
- non-pole balanced tolerance `<= 0.10`
- odd `pi/2` pole-like inputs return finite saturation `+32767.0f` or `-32767.0f`
- dynamic precise-profile tangent is still deferred

Representative balanced resource results:

```text
Phase 48 tan-only shape: roughly 5746 program words
Phase 49 tan-only shape: roughly 3182 program words
__rt_f32_tan:       roughly 449 words
__rt_f32_tan_core:  roughly 2284 words
```

Counts can move slightly with layout changes. The important property is that
tan-only code is materially smaller and does not pull `__rt_f32_sincos_core`.

Pruning rules:

- unused `<math.h>` emits no trig helpers
- constant-folded `tanf` emits no trig helpers
- tan-only emits `__rt_f32_tan` and `__rt_f32_tan_core`
- sin/cos-only emits `__rt_f32_sin`, `__rt_f32_cos`, and `__rt_f32_sincos_core`
- mixed sin/cos/tan emits both cores and reports all wrapper-to-core edges
