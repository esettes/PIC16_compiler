<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Phase 23 Fixed ROM Tables

Phase 23 extends direct ROM indexing to fixed-point calibration tables.

Supported objects:

```c
const __rom __fixed8_8 calibration[] = { 1.0q8_8, 1.5q8_8 };
const __rom __ufixed16_16 gains[] = { 1.0uq16_16, 0.5uq16_16 };
```

Layout:

- fixed values are stored as raw little-endian bytes
- Q8.8/UQ8.8 elements occupy two bytes and lower through `RomRead16`
- Q16.16/UQ16.16 elements occupy four bytes and lower through `RomRead32`
- each ROM object still uses one RETLW byte payload page

Reads:

```c
value = calibration[index];
gain = gains[index];
```

Constant-index reads inline the bytes. Dynamic reads call the RETLW table once per byte and return zero for out-of-range indices.

Limitations:

- ROM fixed objects must be one-dimensional file-scope `const __rom` arrays
- ROM pointers remain unsupported
- fixed ROM structs/unions remain unsupported
- dynamic ROM reads inside ISR code remain rejected unless the index is compile-time constant
- one ROM object must fit within the existing 255-byte RETLW payload limit
