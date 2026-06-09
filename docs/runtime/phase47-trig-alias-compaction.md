<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Phase 47 Trig Alias Compaction

Phase 46 represented positive and negative angle aliases as separate branches. Phase 47 stores one positive magnitude entry per validated angle.

Examples:

```text
+5pi -> +pi behavior
-5pi -> sign-flipped sine, same cosine
+8pi -> 0 behavior
-8pi -> sign-flipped sine zero, same cosine
```

Rules:

- `cos(-x)` reuses `cos(x)`
- `sin(-x)` returns `-sin(x)`
- zero sine stays raw zero in the finite-only model
- no NaN/Inf handling is added
- precise dynamic trig remains deferred

Validated aliases remain:

```text
±3pi
±4pi
±5pi
±6pi
±8pi
```

Reports still show `variant=shared_core_compact` or `variant=shared_core_balanced`; the compaction is internal to that shared core.
