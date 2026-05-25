<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Phase 43 Trig Lowering

IR adds no new node. `sinf` and `cosf` remain direct call IR:

```text
sinf(x) -> call sinf
cosf(x) -> call cosf
```

Backend recognizes those symbols and lowers dynamic calls to:

```text
sinf -> __rt_f32_sin
cosf -> __rt_f32_cos
```

Folded constants disappear before backend lowering and do not emit helpers or ROM math tables.

`--math-profile compact` selects `variant=table_compact`. Default `balanced` selects `variant=table_balanced`. `precise` dynamic trig is deferred in Phase 43 and diagnoses rather than falling back.
