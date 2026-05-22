<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Phase 37 `sqrtf` Lowering

`sqrtf` remains an ordinary direct call in IR. The backend recognizes the symbol and lowers dynamic calls to:

```text
sqrtf -> __rt_f32_sqrt
```

Constant finite calls fold before IR lowering. Negative constant input folds to raw `0.0f`, matching runtime behavior.

Dynamic calls use the existing Stack-first ABI:

- argument: one 4-byte raw f32 value
- return: existing 32-bit return convention (`W`, `return_high`, `return_upper0`, `return_upper1`)
- helper label is emitted only when a dynamic call survives folding and pruning

Phase 38 makes helper selection math-profile-aware. `compact` and default `balanced` lower dynamic calls to the compact `__rt_f32_sqrt` variant. Phase 39 makes `precise` lower to the larger refined `__rt_f32_sqrt` variant instead of diagnosing it as deferred.
