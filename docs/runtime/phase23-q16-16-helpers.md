<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Phase 23 Q16.16 Helpers

Phase 23 folds Q16.16/UQ16.16 constant multiply/divide exactly:

```text
mul: raw_result = (raw_a * raw_b) >> 16
div: raw_result = (raw_a << 16) / raw_b
```

The public C type system still does not expose `long long` or a general 64-bit integer type.

Runtime status:

- dynamic Q16.16 multiply/divide are rejected in semantic analysis in this Phase 23 repository state
- helper labels are reserved in the backend model as `__rt_mul_q16_16`, `__rt_mul_uq16_16`, `__rt_div_q16_16`, and `__rt_div_uq16_16`
- the dynamic helper path remains deferred because the correct helper body is large enough to cross PIC14 program pages, and the current local-branch stub strategy must be split or paged before this is safe

This is intentional: Phase 23 must not silently emit incorrect page-crossing helper code. Use Q8.8 runtime multiply/divide, Q16.16 add/sub/compare/casts, or Q16.16 constant expressions until the helper split is implemented.
