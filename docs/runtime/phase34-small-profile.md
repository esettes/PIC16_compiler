<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Phase 34 Small Runtime Profile

`--runtime-profile small` is now more than an early-warning profile. It selects compact helper variants for the first supported helper family: 32-bit integer division/modulo.

## Behavior

```bash
picc --target pic16f877a -I include --runtime-profile small --size --memory-report -o build/app.hex app.c
```

For code using both `/` and `%` on 32-bit integers, `small` emits:

```text
__rt_div_u32 -> __rt_u32_divmod_core
__rt_mod_u32 -> __rt_u32_divmod_core
```

Signed wrappers use the same unsigned core after sign normalization.

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

## Stack Tradeoff

Compact helpers may add helper-to-helper calls. Use:

```bash
picc --stack-report --runtime-profile small ...
```

The stack report includes helper extra cost so size savings do not hide stack pressure.

## Fast Profile

`--runtime-profile fast` remains reserved and currently follows balanced helper selection.
