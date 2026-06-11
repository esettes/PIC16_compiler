<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Phase 50 Trig Runtime Strategies

The backend selects trig runtime strategy from actually used math builtins.

Selection:

```text
only sinf/cosf:
  strategy=sincos_shared_core
  emit __rt_f32_sincos_core

only tanf:
  strategy=isolated_tan_core
  emit __rt_f32_tan_core

sinf/cosf plus tanf:
  strategy=combined_sincos_tan_core
  emit one __rt_f32_sincos_core with TAN mode
```

The strategy is visible in:

- `--size`
- `--memory-report`
- `.map`
- `.lst` helper variant comments

Combined strategy changes dependency reporting:

```text
__rt_f32_tan -> __rt_f32_sincos_core
```

Tan-only dependency reporting remains:

```text
__rt_f32_tan -> __rt_f32_tan_core
```

All wrapper-to-core calls use existing page-safe helper call emission. Stack
analysis uses the selected strategy for helper-to-helper depth. Layout
validation still rejects unsafe cross-page edges.
