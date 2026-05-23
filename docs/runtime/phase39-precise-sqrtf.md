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

It keeps the Phase 37 exact cases and adds refined raw f32 results for simulator-validated roots. Phase 40 expands the table:

```text
sqrtf(0.25f) -> 0x3F000000
sqrtf(0.5f)  -> 0x3F3504F3
sqrtf(1.0f)  -> 0x3F800000
sqrtf(2.0f)  -> 0x3FB504F3
sqrtf(3.0f)  -> 0x3FDDB3D7
sqrtf(4.0f)  -> 0x40000000
sqrtf(5.0f)  -> 0x400F1BBD
sqrtf(8.0f)  -> 0x403504F3
sqrtf(9.0f)  -> 0x40400000
sqrtf(10.0f) -> 0x404A62C2
sqrtf(16.0f) -> 0x40800000
sqrtf(25.0f) -> 0x40A00000
sqrtf(36.0f) -> 0x40C00000
sqrtf(49.0f) -> 0x40E00000
sqrtf(64.0f) -> 0x41000000
```

The Phase 40 validation policy for the documented positive range is absolute error `±0.03125` versus host `f32::sqrt`. Other positive finite inputs still use the compact fallback. This makes `precise` meaningfully more accurate than `compact` for the documented validated set while keeping the helper within PIC16F877A program memory.

## Cost

The precise variant is larger than compact. Use:

```bash
picc --target pic16f877a -I include --math-profile precise --size --memory-report -o build/sqrt.hex examples/pic16f877a/math_sqrt_precise.c
```

Resource fitting remains authoritative. PIC16F628A may reject precise math-heavy programs.
