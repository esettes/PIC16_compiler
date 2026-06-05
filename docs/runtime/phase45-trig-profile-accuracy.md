<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Phase 45 Trig Profile Accuracy

`--math-profile` controls finite trig accuracy policy:

```text
compact  -> shared_core_compact, tolerance <= 0.10 on validated grid
balanced -> shared_core_balanced, tolerance <= 0.05 on validated grid
precise  -> dynamic trig deferred
```

`compact` and `balanced` both use the Phase 44 shared core. Balanced reports a larger internal ROM table and stricter documented tolerance, but Phase 45 still uses deterministic validated point matches plus coarse fallback. Balanced is tested to be no worse than compact on the validated grid.

Reports include:

```text
Math profile: balanced
Math accuracy: ... tolerance <=0.05 in validated range [-2pi,+2pi] ...
__rt_f32_sincos_core: category=math variant=shared_core_balanced ...
```

Constant folding:

- finite constant `sinf` / `cosf` calls fold with host `f32`
- folded calls emit no trig helper and no trig ROM table
- folded results may be more accurate than dynamic compact/balanced runtime

Remaining limits:

- no full argument reduction
- no `precise` dynamic trig
- no NaN/Inf behavior
- no correctly-rounded IEEE/libm guarantee
