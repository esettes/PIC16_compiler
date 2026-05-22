<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Phase 38 `sqrtf` Accuracy

`sqrtf` remains finite-only:

- no NaN
- no Inf
- no `errno`
- no fenv exceptions
- no full IEEE correctly-rounded guarantee

## Constant Calls

Constant positive finite `sqrtf` calls fold at compile time using Rust `f32` behavior. Negative constants fold to `0.0f`, matching runtime policy.

Constant folding is intentionally independent of math profile unless future precise runtime behavior requires strict profile matching.

## Dynamic Compact/Balanced Calls

Dynamic `sqrtf` under `compact` and current default `balanced` uses the Phase 37 compact helper:

```text
0.0f  -> 0.0f
1.0f  -> 1.0f
4.0f  -> 2.0f
9.0f  -> 3.0f
2.25f -> 1.5f
0.25f -> 0.5f
x < 0 -> 0.0f
```

Other positive finite inputs use the documented compact approximation fallback. This is suitable for small firmware where code size matters more than exact square-root accuracy.

## Dynamic Precise Calls

`--math-profile precise` currently diagnoses dynamic `sqrtf` as unavailable for PIC16 targets. This avoids silently using the compact approximation when the user explicitly asked for a more accurate policy.

Use fixed-point, calibration tables, or compile-time constants when exact square-root behavior matters today.
