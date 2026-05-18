<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Phase 34 Small Runtime Profile

`--runtime-profile small` is now more than an early-warning profile. It selects compact helper variants.

## Behavior

```bash
picc --target pic16f877a -I include --runtime-profile small --size --memory-report -o build/app.hex app.c
```

For code using both `/` and `%` on 32-bit integers, Phase 34 `small` emits:

```text
__rt_div_u32 -> __rt_u32_divmod_core
__rt_mod_u32 -> __rt_u32_divmod_core
```

Signed wrappers use the same unsigned core after sign normalization.

Phase 35 adds more compact wrappers:

```text
__rt_mul_q16_16 -> __rt_mul_uq16_16
__rt_div_q16_16 -> __rt_div_uq16_16
__rt_f32_sub -> __rt_f32_add
```

The Q16.16 wrappers normalize signs before calling the unsigned helper and reapply the result sign. The f32 subtraction wrapper flips the RHS sign bit and calls f32 addition.

## Report Output

`--memory-report` shows the selected variant:

```text
__rt_div_u32: category=division variant=small ... deps=__rt_u32_divmod_core
Runtime Helper Dependency Graph
__rt_div_u32 -> __rt_u32_divmod_core
```

`--size` prints the selected runtime profile and helper word totals.

## Size Example

For `examples/pic16f877a/runtime_profile_small_divmod.c`:

```text
balanced: Program words 3730, runtime helpers 2840
small:    Program words 3659, runtime helpers 2773
```

Exact counts can change as backend lowering evolves. Tests assert the ordering, not fragile exact numbers.

Phase 35 representative results:

```text
Q16 div balanced: Program words 6951, runtime helpers 6037
Q16 div small:    Program words 4981, runtime helpers 4067
f32 add/sub balanced: Program words 8040, runtime helpers 7150
f32 add/sub small:    Program words 5102, runtime helpers 4212
```

## Stack Tradeoff

Compact helpers may add helper-to-helper calls. Use:

```bash
picc --stack-report --runtime-profile small ...
```

The stack report includes helper extra cost so size savings do not hide stack pressure.

## Fast Profile

`--runtime-profile fast` remains reserved and currently follows balanced helper selection.
