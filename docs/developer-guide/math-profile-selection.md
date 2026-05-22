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

Use `precise` only to enforce that compact dynamic `sqrtf` is not used. Dynamic `sqrtf` currently fails with a clear diagnostic under `precise`; constant-folded `sqrtf` still compiles because no runtime helper is emitted.

## Report Workflow

```bash
picc --target pic16f877a -I include --math-profile compact --size --memory-report -o build/math.hex examples/pic16f877a/math_sqrt_profile_compact.c
```

Inspect:

- `Math profile`
- `Runtime helper contribution`
- `variant=compact_approx`
- `math` helper word totals

If precise dynamic `sqrtf` is required, keep the expression constant-folded or defer the firmware change until the precise helper is implemented.
