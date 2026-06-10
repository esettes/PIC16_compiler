<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Phase 48 `tanf` Helper

Runtime helper:

```text
__rt_f32_tan
```

`__rt_f32_tan` is a thin math wrapper. Phase 49 makes it push the raw f32 argument and call a tangent-specific core:

```text
__rt_f32_tan_core
```

The implementation does not duplicate the complete `sinf` / `cosf` runtime and does not pull `__rt_f32_div`. Tangent-specific finite point matching is isolated from `__rt_f32_sincos_core`, so sin/cos-only programs keep the Phase 47 compact core and tan-only programs avoid the larger sine/cosine core.

Reports show:

```text
__rt_f32_tan: category=math variant=wrapper
__rt_f32_tan_core: category=math variant=tan_core_balanced
__rt_f32_tan -> __rt_f32_tan_core
```

No tangent ROM table is emitted in Phase 49. The tangent core uses deterministic finite validated point matches plus the documented saturation policy for odd `pi/2` pole-like inputs.
