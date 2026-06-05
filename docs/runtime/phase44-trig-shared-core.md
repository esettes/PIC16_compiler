<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Phase 44 Trig Shared Core

Phase 44 reduces finite `sinf` / `cosf` runtime duplication.

Runtime helpers:

```text
__rt_f32_sin           wrapper
__rt_f32_cos           wrapper
__rt_f32_sincos_core   shared core
```

The wrappers push the raw f32 input plus a one-byte mode:

```text
mode=0 -> sin
mode=1 -> cos
```

The shared core performs the Phase 43 finite table-point matching and coarse fallback. It does not change accuracy policy:

- compact tolerance remains `<= 0.10` on validated simulator points
- balanced tolerance remains `<= 0.05` on validated simulator points
- precise dynamic trig remains deferred
- no NaN/Inf/errno/fenv/libm behavior is provided

Phase 45 expands simulator validation for the shared core and adds deterministic aliases for common `±3pi` / `±4pi` inputs. It does not add full range reduction.

Pruning rules:

- no dynamic trig call: no wrappers, no shared core, no trig ROM table
- only `sinf`: `__rt_f32_sin` + shared core + selected table
- only `cosf`: `__rt_f32_cos` + shared core + selected table
- both: both wrappers + one shared core + one selected table

Reports show wrapper dependencies:

```text
__rt_f32_sin -> __rt_f32_sincos_core
__rt_f32_cos -> __rt_f32_sincos_core
```

The benefit is program-memory reduction when `sinf` and `cosf` are used together. Cost moves from two large duplicated helpers to small wrappers plus one shared core.
