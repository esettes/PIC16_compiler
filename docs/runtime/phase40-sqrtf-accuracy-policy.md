<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Phase 40 `sqrtf` Accuracy Policy

`sqrtf` remains finite-only:

- negative input returns `0.0f`
- no NaN
- no Inf
- no `errno`
- no fenv exceptions
- no correctly-rounded IEEE-754 guarantee

## Profiles

`compact` uses the small Phase 37 approximation. It is exact for the simple validated values and otherwise has no general accuracy guarantee.

`balanced` currently aliases `compact`.

`precise` uses `variant=precise_table_refined`. Phase 40 expands the refined table and validates the following positive inputs:

```text
0.25, 0.5, 1.0, 2.0, 3.0, 4.0, 5.0, 8.0, 9.0, 10.0, 16.0, 25.0, 36.0, 49.0, 64.0
```

For the validated positive range `[0.25, 64.0]`, simulator tests require absolute error `<= 0.03125` against host `f32::sqrt`.

Outside the refined table, dynamic precise `sqrtf` still uses the compact fallback. This is why the compiler says "more accurate than compact for the validated range", not "IEEE precise".

## Reports

`--size`, `--memory-report`, `.map`, and `.lst` show the selected math profile. `--size` and `--memory-report` also show the selected math accuracy policy:

```text
Math profile: precise
Math accuracy: precise sqrtf: table-refined finite approximation; validated positives in [0.25, 64.0] within +/-0.03125; not IEEE correctly-rounded
```

Use `--math-profile precise` on PIC16F877A when the validated range matters. Use `compact` for smaller targets or when a calibration table/fixed-point implementation gives better control.
