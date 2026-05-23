<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Phase 39 `sqrtf` Resource Cost

`--math-profile precise` increases program-memory cost for `sqrtf`.

Reports show:

```text
Math profile: precise
Math accuracy: precise sqrtf: table-refined finite approximation; validated positives in [0.25, 64.0] within +/-0.03125; not IEEE correctly-rounded
__rt_f32_sqrt: category=math variant=precise_table_refined ...
```

Compare with compact:

```bash
picc --target pic16f877a -I include --math-profile compact --size --memory-report -o build/sqrt-compact.hex examples/pic16f877a/math_sqrt_profile_compare.c
picc --target pic16f877a -I include --math-profile precise --size --memory-report -o build/sqrt-precise.hex examples/pic16f877a/math_sqrt_profile_compare.c
```

Expected behavior:

- precise uses more program words than compact
- both variants remain demand-pruned when unused
- Phase 40 reports the selected math accuracy policy alongside the helper variant
- resource fitting rejects targets that cannot fit the selected helper
- stack reports include the selected helper frame cost

The precise variant is intended for PIC16F877A first. Use compact or fixed-point on smaller targets when resource reports are tight.
