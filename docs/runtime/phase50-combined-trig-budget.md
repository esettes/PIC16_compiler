<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Phase 50 Combined Trig Budget

Phase 50 reduces program-memory pressure when `sinf`, `cosf`, and `tanf` are
all used in one firmware image.

Runtime strategies:

```text
sincos_shared_core:
  __rt_f32_sin -> __rt_f32_sincos_core
  __rt_f32_cos -> __rt_f32_sincos_core

isolated_tan_core:
  __rt_f32_tan -> __rt_f32_tan_core

combined_sincos_tan_core:
  __rt_f32_sin -> __rt_f32_sincos_core
  __rt_f32_cos -> __rt_f32_sincos_core
  __rt_f32_tan -> __rt_f32_sincos_core
```

The combined strategy is selected only when tangent and sine/cosine are used
together. This preserves Phase 49 single-use costs:

- sin/cos-only does not emit tangent helpers
- tan-only does not emit sine/cosine wrappers or `__rt_f32_sincos_core`
- constant-folded trig emits no trig helpers

Representative balanced PIC16F877A sizes:

```text
sin/cos-only:       about 4982 / 8192 words
tan-only:           about 3161 / 8192 words
sin/cos/tan mixed:  about 7250 / 8192 words
Phase 49 mixed:     about 8005 / 8192 words
```

The preferred `<=7200` target is not reached without changing the generated
point-matcher shape further. Phase 50 still saves roughly 755 program words and
leaves substantially more headroom than the split-core Phase 49 mixed case.

Behavior is unchanged:

- finite-only
- radians input
- no NaN, Inf, errno, or fenv
- no libm/correctly-rounded claim
- tangent pole-like points return finite `+32767.0f` or `-32767.0f`
- dynamic precise trig remains deferred
