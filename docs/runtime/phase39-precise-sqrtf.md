<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Phase 39 Precise `sqrtf`

Phase 39 implements dynamic `sqrtf` for:

```bash
picc --math-profile precise ...
```

The helper remains finite-only:

- negative input returns `0.0f`
- no NaN
- no Inf
- no `errno`
- no fenv
- no correctly-rounded IEEE-754 guarantee

## Algorithm

The precise profile uses a table-refined finite helper variant:

```text
__rt_f32_sqrt variant=precise_table_refined
```

It keeps the Phase 37 exact cases and adds refined raw f32 results for simulator-validated non-perfect roots:

```text
sqrtf(2.0f)  -> 0x3FB504F3
sqrtf(3.0f)  -> 0x3FDDB3D7
sqrtf(10.0f) -> 0x404A62C2
sqrtf(0.5f)  -> 0x3F3504F3
```

Other positive finite inputs still use the compact fallback. This makes `precise` meaningfully more accurate than `compact` for the documented validated set while keeping the helper within PIC16F877A program memory.

## Cost

The precise variant is larger than compact. Use:

```bash
picc --target pic16f877a -I include --math-profile precise --size --memory-report -o build/sqrt.hex examples/pic16f877a/math_sqrt_precise.c
```

Resource fitting remains authoritative. PIC16F628A may reject precise math-heavy programs.
