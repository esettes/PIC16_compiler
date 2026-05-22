<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Phase 39 `sqrtf` Profile Lowering

The frontend and IR do not add new syntax. `sqrtf` remains a direct call in IR:

```text
sqrtf -> __rt_f32_sqrt
```

Backend lowering selects the helper body from `--math-profile`:

```text
compact  -> __rt_f32_sqrt variant=compact_approx
balanced -> __rt_f32_sqrt variant=compact_approx
precise  -> __rt_f32_sqrt variant=precise_table_refined
```

Constant finite calls still fold before backend helper selection. Folded calls do not emit `__rt_f32_sqrt`.

Dynamic helper-backed `sqrtf` remains rejected inside ISRs for all math profiles.
