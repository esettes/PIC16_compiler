<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Phase 23 Fixed Literals

Phase 23 adds explicit decimal fixed-point literals. These are not general floating-point literals.

Supported suffixes:

```text
q8_8
uq8_8
q16_16
uq16_16
```

Examples:

```c
__fixed8_8 a = 1.5q8_8;
__ufixed8_8 b = 2.25uq8_8;
__fixed16_16 c = 10.125q16_16;
__ufixed16_16 d = 0.5uq16_16;
```

Raw conversion is deterministic truncation:

```text
raw = integer_part << fractional_bits
raw += floor(fractional_part * 2^fractional_bits)
```

Examples:

```text
1.5q8_8    -> 0x0180
2.25q8_8   -> 0x0240
1.5q16_16  -> 0x00018000
```

Diagnostics:

- malformed literals such as `1.q8_8`
- missing suffix on decimal fixed literals
- unsupported suffixes such as `q4_4`
- values outside the selected raw fixed range
- implicit unsafe fixed narrowing under `-Werror`

Q16.16 multiply/divide are constant-folded when both operands are compile-time constants. Phase 24 accepts dynamic Q16.16/UQ16.16 multiply/divide outside ISRs and lowers them through runtime helpers.
