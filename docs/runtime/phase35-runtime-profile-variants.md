<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Phase 35 Runtime Profile Variants

Runtime helper variants are selected from the active runtime profile.

## balanced

Default behavior. Emits standalone helper bodies.

## small

Uses compact wrappers where implemented:

```text
__rt_div_u32 -> __rt_u32_divmod_core
__rt_mod_u32 -> __rt_u32_divmod_core
__rt_div_i32 -> __rt_u32_divmod_core
__rt_mod_i32 -> __rt_u32_divmod_core
__rt_mul_q16_16 -> __rt_mul_uq16_16
__rt_div_q16_16 -> __rt_div_uq16_16
__rt_f32_sub -> __rt_f32_add
```

Reports show:

```text
variant=small
deps=...
```

Shared wrappers reduce program words when both the wrapper and its dependency are useful, but they can add helper-to-helper stack depth. Use `--stack-report` for tight RAM targets.

## fast

Reserved. It currently follows `balanced` helper selection.
