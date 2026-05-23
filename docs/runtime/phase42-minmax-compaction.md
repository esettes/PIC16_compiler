# Phase 42 Min/Max Compaction

Phase 42 keeps the Phase 41 finite `fminf` / `fmaxf` public API but removes separate runtime helper bodies.

## Lowering

Dynamic calls lower at the call site:

```text
fminf(a, b) -> __rt_f32_cmp(a, b) + local select
fmaxf(a, b) -> __rt_f32_cmp(a, b) + local select
```

No `__rt_f32_min` or `__rt_f32_max` helper is emitted. Constant-folded calls still emit no helper at all.

Selection rule:

- `fminf` returns `b` only when compare reports `a > b`; otherwise returns `a`
- `fmaxf` returns `b` only when compare reports `a < b`; otherwise returns `a`

## Profiles

Runtime profiles `balanced`, `small`, and `fast` use the same compact finite compare/select semantics. `small` still benefits from helper pruning and the smaller compare helper. Math profiles do not change min/max behavior.

## ISR

Dynamic `fminf` / `fmaxf` remain rejected in ISRs because they require `__rt_f32_cmp`. Constant-folded calls follow the existing finite math folding policy.
