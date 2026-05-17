<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Runtime Profiles

Phase 33 adds:

```bash
picc --runtime-profile small ...
picc --runtime-profile balanced ...
picc --runtime-profile fast ...
```

`balanced` is the default.

## small

Warns earlier when runtime helpers dominate program memory. Use this for `PIC16F628A` or tight firmware.

## balanced

Default policy. It preserves current helper bodies and emits budget warnings only for very large helper sets.

## fast

Reserved for future helper selection. In Phase 33 it uses the same helper bodies and warning threshold as `balanced`.

## Recommended Workflow

```bash
picc --target pic16f877a -I include --runtime-profile small --size --memory-report -o build/app.hex app.c
```

Read:

- `Runtime helpers`
- `Runtime Helper Contributors`
- `Runtime Helper Dependency Graph`
- `.map` runtime helper category groups

If helper cost is too high, prefer fixed-point, remove unused floating-point operations, or select a larger target.
