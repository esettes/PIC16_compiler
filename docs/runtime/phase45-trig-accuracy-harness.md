<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Phase 45 Trig Accuracy Harness

Phase 45 extends simulator numeric validation for finite `sinf` / `cosf`; Phase 48 adds tangent cases to the same style of harness.

Harness flow:

1. generate C with dynamic `sinf` / `cosf` / `tanf` calls
2. compile through `picc`
3. run generated HEX in `pic16-sim`
4. read raw f32 result globals from RAM
5. compare against host `f32::sin` / `f32::cos`
6. apply profile tolerance

Tolerance:

```text
compact:  absolute error <= 0.10
balanced: absolute error <= 0.05
precise:  dynamic trig deferred
```

Validated grid:

```text
0
pi/6
pi/4
pi/3
pi/2
2pi/3
3pi/4
5pi/6
pi
-pi/6
-pi/4
-pi/2
-pi
2pi
-2pi
```

Phase 46 extends common alias checks:

```text
3pi
-3pi
4pi
-4pi
5pi
-5pi
6pi
-6pi
8pi
-8pi
```

Phase 47 keeps the same harness and validates the sign-normalized compact core against the same points. Phase 48 validates non-pole tangent values against host `f32::tan` and validates pole-like points against the finite saturation policy.

This harness proves documented finite behavior on tested inputs only. It does not prove libm-grade argument reduction.
