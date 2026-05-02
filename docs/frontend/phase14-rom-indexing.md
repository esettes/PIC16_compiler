<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Phase 14 ROM Indexing

Phase 14 makes explicit `__rom` arrays usable from ordinary expressions without adding ROM pointers.

## Supported

- file-scope `const __rom char[]`
- file-scope `const __rom unsigned char[]`
- file-scope `const __rom int[]`
- file-scope `const __rom unsigned int[]`
- direct `rom_array[index]` reads
- `__rom_read8(table, index)` for byte ROM arrays
- `__rom_read16(table, index)` for 16-bit ROM arrays
- constant-index ROM reads in normal code and inside ISR when the whole read stays inline-safe

## Rules

- ROM arrays stay in a separate program-memory address space
- ROM arrays do not decay to data-space pointers
- taking the address of one ROM element is rejected
- writing to ROM indexed elements is rejected
- 16-bit ROM elements are packed little-endian across two RETLW payload bytes

## Restrictions

- ROM pointers are still unsupported
- multidimensional `__rom` arrays are still unsupported
- dynamic ROM reads inside ISR are rejected
- ROM structs, unions, and bitfield objects are rejected
