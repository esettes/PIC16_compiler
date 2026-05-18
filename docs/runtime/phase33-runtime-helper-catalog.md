<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Phase 33 Runtime Helper Catalog

Phase 33 centralizes runtime helper metadata in `RuntimeHelper`. Phase 34 uses that metadata for profile-selected helper variants.

Each helper records:

- label
- category
- ABI argument bytes
- local bytes
- stack frame bytes
- required-by text
- estimated words
- dependency list
- page-sensitive flag
- target constraints

Categories are:

- integer
- division
- fixed
- float
- conversion
- shift

The catalog feeds codegen, stack estimates, memory reports, map grouping, listing comments, and diagnostics.

## Dependency Graph

The graph is validated before helper emission so shared helper primitives cannot introduce unknown or cyclic dependencies silently.

Phase 34 adds the first profile-dependent dependencies:

```text
--runtime-profile small
__rt_div_u32 -> __rt_u32_divmod_core
__rt_mod_u32 -> __rt_u32_divmod_core
__rt_div_i32 -> __rt_u32_divmod_core
__rt_mod_i32 -> __rt_u32_divmod_core
```

Phase 35 adds:

```text
--runtime-profile small
__rt_mul_q16_16 -> __rt_mul_uq16_16
__rt_div_q16_16 -> __rt_div_uq16_16
__rt_f32_sub -> __rt_f32_add
```

Under `balanced`, those helpers keep standalone bodies and have no compact-profile dependency.

## Pruning

Helpers are emitted only when codegen marks a concrete runtime use. Declarations and folded constants do not pull helper bodies.

Examples that should stay helper-free:

```c
float f = 1.5f;
long l = 3L;
__fixed16_16 q = 1.0q16_16;
```

## Reports

`--memory-report` includes:

```text
Runtime Helper Contributors
Runtime Helper Dependency Graph
```

For compact helpers, contributor lines include the selected variant and dependency list:

```text
__rt_div_u32: category=division variant=small ... deps=__rt_u32_divmod_core
__rt_f32_sub: category=float variant=small ... deps=__rt_f32_add
```

`.lst` helper bodies include comments such as:

```text
; runtime helper: float helper=__rt_f32_cmp required_by=finite f32 arithmetic/comparison
```
