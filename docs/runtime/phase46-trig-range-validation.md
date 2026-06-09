<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Phase 46 Trig Range Validation

Simulator validation extends the Phase 45 trig harness.

Validated dynamic inputs:

```text
0
pi/6
pi/4
pi/3
pi/2
pi
2pi
3pi
4pi
5pi
6pi
8pi
-3pi
-4pi
-5pi
-6pi
-8pi
```

The harness compiles generated C, executes the HEX with `pic16-sim`, reads raw f32 globals from RAM, and compares against host `f32::sin` / `f32::cos`.

Tolerance:

```text
compact:  absolute error <= 0.10
balanced: absolute error <= 0.05
precise:  dynamic trig deferred
```

Constant folded `sinf` / `cosf` still use host `f32` and emit no trig helper or ROM table, so folded constants may be more accurate than dynamic compact/balanced runtime.

Phase 47 reuses this validation to prove alias compaction did not change Phase 46 behavior.
