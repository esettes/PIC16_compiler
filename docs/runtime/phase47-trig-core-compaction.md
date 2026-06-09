<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Phase 47 Trig Core Compaction

Phase 47 reduces the shared `sinf` / `cosf` runtime core after Phase 46 expanded aliases through `±8pi`.

Phase 46 cost reference:

```text
__rt_f32_sincos_core: 4429 words
math helpers total:   5377 words
```

Phase 47 strategy:

- compare positive magnitude bytes once
- store the input sign in a local flag
- map cosine directly from the positive magnitude result
- map sine through the positive result or sign-flipped result based on the sign flag
- keep one shared `__rt_f32_sincos_core`
- keep small `__rt_f32_sin` / `__rt_f32_cos` wrappers

Balanced example result:

```text
__rt_f32_sincos_core: 3315 words
math helpers total:   4263 words
```

Behavior is unchanged from Phase 46. This remains finite table matching plus coarse fallback, not libm range reduction.
