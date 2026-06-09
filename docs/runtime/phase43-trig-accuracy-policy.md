<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Phase 43 Trig Accuracy Policy

Phase 43 `sinf` / `cosf` and Phase 48 `tanf` are finite approximations:

- radians input
- no NaN
- no Inf
- no errno
- no fenv
- no correctly-rounded IEEE promise
- no full argument reduction

Validated dynamic points are within `[-2pi, +2pi]`:

```text
0
±pi/6
±pi/4
±pi/3
±pi/2
±2pi/3
±3pi/4
±5pi/6
±pi
±3pi/2
±2pi
```

Phase 46 also tests scoped moderate common-multiple aliases:

```text
±3pi
±4pi
±5pi
±6pi
±8pi
```

Those aliases are deterministic point matches, not continuous argument reduction.

Tolerance:

```text
compact:  absolute error <= 0.10 on validated simulator points
balanced: absolute error <= 0.05 on validated simulator points
precise:  dynamic sinf/cosf/tanf deferred
```

Phase 48 tangent uses separate non-pole tolerances:

```text
compact tanf:  absolute error <= 0.20
balanced tanf: absolute error <= 0.10
```

Odd `pi/2` pole-like tangent inputs return finite `±32767.0f` saturation and are tested as policy points, not against host `tan`.

Constant folded trig may be more accurate because semantic folding uses host `f32::sin` / `f32::cos`.

Outside documented points, dynamic fallback is coarse and finite. Use ROM calibration tables or fixed-point if exact behavior matters.
