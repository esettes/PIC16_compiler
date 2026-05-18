<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Runtime Profiles

Phase 33 adds runtime profiles. Phase 34 makes `small` change helper selection for the first compact helper family:

```bash
picc --runtime-profile small ...
picc --runtime-profile balanced ...
picc --runtime-profile fast ...
```

`balanced` is the default.

## small

Uses compact runtime helper variants where implemented and warns earlier when helpers dominate program memory. Phase 34 compacts 32-bit integer division/modulo by using shared `__rt_u32_divmod_core` plus smaller wrappers.

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
```

If helper cost is too high, prefer fixed-point, remove unused floating-point operations, or select a larger target.
