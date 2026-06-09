<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Phase 48 `tanf` Accuracy Policy

`tanf` is finite and approximate.

Policy:

- radians input
- no NaN
- no Inf
- no errno
- no fenv
- no correctly-rounded IEEE/libm claim
- no full argument reduction

Validated non-pole points include `0`, `pi/6`, `pi/4`, `-pi/4`, `pi/3`, `-pi/3`, `pi`, `-pi`, `3pi`, and `-3pi`.

Tolerances:

```text
compact:  absolute error <= 0.20 for non-pole validated points
balanced: absolute error <= 0.10 for non-pole validated points
```

Pole policy:

```text
tanf(pi/2)  -> +32767.0f
tanf(-pi/2) -> -32767.0f
```

Odd `pi/2` aliases use the sign of the original input and return finite saturation. This avoids unsupported Inf/NaN behavior.

Constant folded `tanf` uses host `f32::tan` for finite values and clamps non-finite results to the same finite saturation policy. Folded constants may be more accurate than dynamic compact/balanced runtime values.
