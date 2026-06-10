<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Phase 49 Trig Helper Splitting

Phase 49 splits tangent codegen into a separate math helper section:

```text
__rt_f32_tan      category=math variant=wrapper
__rt_f32_tan_core category=math variant=tan_core_balanced
```

The runtime catalog declares:

```text
__rt_f32_tan -> __rt_f32_tan_core
```

`__rt_f32_sin` and `__rt_f32_cos` keep their existing dependency:

```text
__rt_f32_sin -> __rt_f32_sincos_core
__rt_f32_cos -> __rt_f32_sincos_core
```

All wrapper-to-core calls use the normal page-safe runtime-helper call path.
The linker layout validator therefore sees tangent helper-to-helper calls just
like other runtime dependencies.

Map and memory reports group `__rt_f32_tan_core` under math helpers. Listings
show the standalone tangent core body; sin/cos-only listings do not contain it.

This phase does not add a tangent ROM table. The tangent core uses compact
finite point matching and finite saturation. It intentionally avoids pulling the
full float divide helper.
