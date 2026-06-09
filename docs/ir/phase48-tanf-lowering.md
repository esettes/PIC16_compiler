<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Phase 48 `tanf` Lowering

IR adds no new node:

```text
tanf(x) -> direct call IR
```

The PIC16 backend recognizes the symbol and lowers dynamic calls to:

```text
__rt_f32_tan wrapper -> __rt_f32_sincos_core(mode=2)
```

Folded constants disappear before IR/backend lowering and emit no trig wrapper, no shared core, and no internal ROM table.

`fminf` / `fmaxf`, `sinf`, and `cosf` lowering remain unchanged.
