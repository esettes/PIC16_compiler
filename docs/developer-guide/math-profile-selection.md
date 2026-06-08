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

Phase 41 `fminf` / `fmaxf` are not accuracy-profile-dependent. Phase 42 lowers them to compact `__rt_f32_cmp` plus local select and returns one of the original operands.

Phase 44 `sinf` / `cosf` use the math profile and share one runtime core:

- `compact`: `variant=shared_core_compact`, smaller internal ROM quarter-wave table, validated tolerance `<= 0.10`
- `balanced`: default `variant=shared_core_balanced`, larger internal ROM quarter-wave table, validated tolerance `<= 0.05`
- `precise`: dynamic `sinf` / `cosf` is deferred and diagnoses; constant folded calls still fold at compile time

Phase 46 validates compact and balanced trig over the documented point grid in `[-2pi,+2pi]` plus scoped common aliases through `±8pi`. Balanced uses the same shared-core point matcher and must be no worse than compact on the validated grid; it does not imply continuous or libm-grade argument reduction.

Runtime profile still controls helper sharing/size strategy. Math profile controls numerical policy. Do not use `--math-profile precise` for dynamic trig in Phase 46.

## Report Workflow

```bash
picc --target pic16f877a -I include --math-profile compact --size --memory-report -o build/math.hex examples/pic16f877a/math_sqrt_profile_compact.c
```

Inspect:

- `Math profile`
- `Runtime helper contribution`
- `variant=compact_approx`
- `variant=precise_table_refined`
- `variant=shared_core_compact`
- `variant=shared_core_balanced`
- `Math accuracy`
- `math` helper word totals

If exact square-root behavior is required outside the documented refined table, keep the expression constant-folded, use fixed-point/calibration tables, or validate the specific operating range in simulator.
