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

Phase 39 implements `--math-profile precise` for dynamic `sqrtf` with a larger refined helper. It adds exact raw f32 results for additional simulator-validated non-perfect roots:

```text
2.0f  -> 1.4142135f
3.0f  -> 1.7320508f
10.0f -> 3.1622777f
0.5f  -> 0.70710677f
```

Other positive finite inputs still use the compact fallback. This makes precise meaningfully better than compact for the documented set, but it is not a general correctly-rounded IEEE square-root implementation.
