<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Phase 35 Fixed/Float Helper Compaction

Phase 35 extends `--runtime-profile small` beyond 32-bit integer div/mod.

## Q16.16

Compact signed helpers depend on unsigned helpers:

```text
__rt_mul_q16_16 -> __rt_mul_uq16_16
__rt_div_q16_16 -> __rt_div_uq16_16
```

The signed wrapper:

- normalizes both signs in-place
- calls the unsigned helper using the Stack-first ABI
- reapplies the final sign when needed
- preserves dynamic divide-by-zero behavior: raw `0`

## Float

Compact float subtraction depends on float addition:

```text
__rt_f32_sub -> __rt_f32_add
```

The wrapper flips the RHS sign bit and calls the finite add helper. This preserves the existing finite-only policy and avoids duplicating the full add/sub Q16.16 bridge.

## Not Compacted Yet

- Q8.8 helpers
- Q16.16 unsigned helper internals
- float add/mul/div internals
- float compare internals
- float/fixed/integer conversion internals
- generic f32 pack/unpack primitives

Those remain standalone because sharing them safely needs more runtime and simulator work.

Phase 36 builds on this catalog by adding a separate `math` helper category for `fabsf`, `truncf`, `floorf`, `ceilf`, and `roundf`. These helpers are pruned and reported through the same dependency graph and resource-report path.
