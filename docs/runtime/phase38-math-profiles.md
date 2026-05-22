<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Phase 38 Math Profiles

Phase 38 adds a math accuracy/resource policy independent from runtime helper sharing:

```bash
picc --math-profile compact ...
picc --math-profile balanced ...
picc --math-profile precise ...
```

## Profiles

`compact` keeps the Phase 37 finite `sqrtf` helper:

- exact for documented simulator-validated values
- approximate fallback for other positive finite values
- negative inputs return `0.0f`
- smallest currently implemented dynamic `sqrtf` path

`balanced` is the default. It currently uses the same compact `sqrtf` helper and reports `variant=compact_approx`.

`precise` is an explicit request for the larger refined helper added in Phase 39. It reports `variant=precise_table_refined` and is more accurate than compact for the simulator-validated non-perfect roots documented in the Phase 39 runtime notes.

## Runtime Profile Difference

`--runtime-profile small|balanced|fast` chooses helper sharing and code-size strategy. `--math-profile compact|balanced|precise` chooses numeric accuracy policy.

Example:

```bash
picc --target pic16f877a -I include --runtime-profile small --math-profile compact --size --memory-report -o build/app.hex app.c
```

## Reports

`--size`, `--memory-report`, `.map`, and `.lst` show the selected math profile. Dynamic `sqrtf` in compact/balanced reports:

```text
__rt_f32_sqrt: category=math variant=compact_approx ...
```

Dynamic `sqrtf` in precise reports:

```text
__rt_f32_sqrt: category=math variant=precise_table_refined ...
```

Constant `sqrtf` calls fold before helper emission and do not emit `__rt_f32_sqrt`.
