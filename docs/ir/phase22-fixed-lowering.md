<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Phase 22 Fixed Lowering

Fixed-point values reuse existing scalar IR storage widths:

- Q8.8/UQ8.8 lower as 16-bit raw values
- Q16.16/UQ16.16 lower as 32-bit raw values

The IR keeps `Type` metadata so casts and comparisons preserve fixed signedness and raw width.

## Casts

Integer-to-fixed:

```text
raw = integer << fractional_bits
```

Fixed-to-integer:

```text
integer = raw >> fractional_bits
```

Fixed-to-fixed casts adjust raw fractional width with shifts, then extend or truncate to the target raw width.

## Arithmetic

Add/subtract use raw integer add/subtract and preserve fixed type.

Comparisons lower to comparisons over the raw integer type:

- Q8.8 -> `int`
- UQ8.8 -> `unsigned int`
- Q16.16 -> `long`
- UQ16.16 -> `unsigned long`

Q8.8 multiply lowers to:

```text
wide = raw_a_32 * raw_b_32
raw_result = wide >> 8
```

Q8.8 divide lowers to:

```text
wide = raw_a_32 << 8
raw_result = wide / raw_b_32
```

Phase 23 folds Q16.16 multiply/divide constants before backend emission. Dynamic Q16.16 multiply/divide are rejected before IR generation.

Phase 23 fixed ROM tables lower direct indexing as:

- Q8.8/UQ8.8 -> `RomRead16`
- Q16.16/UQ16.16 -> `RomRead32`
