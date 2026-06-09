<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Phase 43 Trig Lowering

IR adds no new node. `sinf`, `cosf`, and Phase 48 `tanf` remain direct call IR:

```text
sinf(x) -> call sinf
cosf(x) -> call cosf
tanf(x) -> call tanf
```

Backend recognizes those symbols and lowers dynamic calls to:

```text
sinf -> __rt_f32_sin wrapper -> __rt_f32_sincos_core
cosf -> __rt_f32_cos wrapper -> __rt_f32_sincos_core
tanf -> __rt_f32_tan wrapper -> __rt_f32_sincos_core
```

Folded constants disappear before backend lowering and do not emit helpers or ROM math tables.

`--math-profile compact` selects `variant=shared_core_compact`. Default `balanced` selects `variant=shared_core_balanced`. `precise` dynamic trig is deferred and diagnoses rather than falling back.
