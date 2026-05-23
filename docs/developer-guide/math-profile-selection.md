<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Math Profile Selection

Use `--math-profile` to choose finite math accuracy policy:

```bash
picc --target pic16f877a -I include --math-profile compact -o build/app.hex app.c
picc --target pic16f877a -I include --math-profile balanced -o build/app.hex app.c
picc --target pic16f877a -I include --math-profile precise -o build/app.hex app.c
```

## Recommendations

Use `compact` on PIC16F628A or size-sensitive PIC16F877A firmware. It uses the compact dynamic `sqrtf` approximation and reports that choice.

Use `balanced` when you want defaults. Today it is equivalent to `compact` for `sqrtf`.

Use `precise` when the firmware needs the larger refined dynamic `sqrtf` helper. It is more accurate than compact for the documented validated roots, but still finite-only and not correctly-rounded IEEE.

Phase 40 documents the tested precise range as positive inputs in `[0.25, 64.0]` with absolute tolerance `±0.03125` against host `f32::sqrt`. Inputs outside the refined table can still use the compact fallback.

Phase 41 `fminf` / `fmaxf` are not accuracy-profile-dependent. They use finite comparisons and return one of the original operands.

## Report Workflow

```bash
picc --target pic16f877a -I include --math-profile compact --size --memory-report -o build/math.hex examples/pic16f877a/math_sqrt_profile_compact.c
```

Inspect:

- `Math profile`
- `Runtime helper contribution`
- `variant=compact_approx`
- `variant=precise_table_refined`
- `Math accuracy`
- `math` helper word totals

If exact square-root behavior is required outside the documented refined table, keep the expression constant-folded, use fixed-point/calibration tables, or validate the specific operating range in simulator.
