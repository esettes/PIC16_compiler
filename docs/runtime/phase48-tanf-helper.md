<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Phase 48 `tanf` Helper

Runtime helper:

```text
__rt_f32_tan
```

`__rt_f32_tan` is a thin math wrapper. It pushes the raw f32 argument plus mode byte `2`, then calls:

```text
__rt_f32_sincos_core
```

The implementation does not duplicate the complete `sinf` / `cosf` runtime and does not pull `__rt_f32_div`. The shared core emits TAN-mode branches only when `tanf` is used, so sin/cos-only programs keep the Phase 47 compact core.

Reports show:

```text
__rt_f32_tan: category=math variant=wrapper
__rt_f32_tan -> __rt_f32_sincos_core
```

The selected internal ROM sine quarter-wave table is reused; no tangent ROM table is emitted in Phase 48.
