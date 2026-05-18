<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Runtime Profiles

Phase 33 adds runtime profiles. Phase 34 makes `small` change helper selection for 32-bit div/mod. Phase 35 extends `small` to selected Q16.16 and float helpers. Phase 36/37 math helpers reuse the same catalog, dependency graph, and report machinery.

```bash
picc --runtime-profile small ...
picc --runtime-profile balanced ...
picc --runtime-profile fast ...
```

`balanced` is the default.

## small

Uses compact runtime helper variants where implemented and warns earlier when helpers dominate program memory.

Implemented compact families:

- 32-bit integer division/modulo wrappers use shared `__rt_u32_divmod_core`
- signed Q16.16 multiply/divide wrappers use unsigned Q16.16 helpers
- finite f32 subtraction uses f32 addition after flipping the RHS sign bit

Use this for tight firmware after checking stack cost. Shared primitives can reduce program words while adding helper-to-helper calls.

## balanced

Default policy. It preserves standalone helper bodies and emits budget warnings only for very large helper sets.

## fast

Reserved for future helper selection. It currently uses the same helper bodies and warning threshold as `balanced`.

## Recommended Workflow

```bash
picc --target pic16f877a -I include --runtime-profile small --size --memory-report -o build/app.hex app.c
```

Read:

- `Runtime helpers`
- `Runtime Helper Contributors`
- `Runtime Helper Dependency Graph`
- `.map` runtime helper category groups

For compact helpers, expect lines like:

```text
__rt_div_u32: category=division variant=small ... deps=__rt_u32_divmod_core
__rt_u32_divmod_core -> (none)
__rt_div_q16_16: category=fixed variant=small ... deps=__rt_div_uq16_16
__rt_f32_sub: category=float variant=small ... deps=__rt_f32_add
```

If helper cost is too high, prefer fixed-point, remove unused floating-point operations, or select a larger target.
For math-heavy firmware, inspect `math helper` entries separately; `floorf`, `ceilf`, `roundf`, and `sqrtf` depend on conversion/comparison/arithmetic helpers.
