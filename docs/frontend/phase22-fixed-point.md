<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Phase 22 Fixed-Point Types

Phase 22 adds explicit fixed-point scalar keywords:

- `__fixed8_8`: signed Q8.8, 16-bit raw storage
- `__ufixed8_8`: unsigned UQ8.8, 16-bit raw storage
- `__fixed16_16`: signed Q16.16, 32-bit raw storage
- `__ufixed16_16`: unsigned UQ16.16, 32-bit raw storage

Storage is little-endian. `sizeof(__fixed8_8)` and `sizeof(__ufixed8_8)` are 2. `sizeof(__fixed16_16)` and `sizeof(__ufixed16_16)` are 4.

Fixed values are supported in globals, file-scope statics, static locals, auto locals, parameters, returns, arrays, structs, unions, and data pointers.

## Constants

Phase 22 intentionally does not parse decimal fixed literals. Use raw constructors:

- `__q8_8(raw)`
- `__uq8_8(raw)`
- `__q16_16(raw)`
- `__uq16_16(raw)`

Example:

```c
__fixed8_8 one_half = __q8_8(0x0080);
__fixed8_8 three = (__fixed8_8)3;
```

Integer-to-fixed casts shift left by the fractional bit count. Fixed-to-integer casts shift right by the fractional bit count.

## Operations

Supported:

- Q8.8/UQ8.8: `+`, `-`, `*`, `/`, comparisons, unary `-`, casts
- Q16.16/UQ16.16: `+`, `-`, comparisons, unary `-`, casts

Rejected:

- Q16.16/UQ16.16 `*` and `/`
- fixed `%`
- bitwise fixed operators unless the program casts to a raw integer first
- fixed ROM objects
- helper-backed fixed operations inside ISR code

Implicit fixed/integer conversions and fixed narrowing warn. Under `-Werror`, unsafe implicit fixed conversions fail.
