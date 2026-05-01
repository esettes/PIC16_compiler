<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Phase 3 PIC16 Memory Model

## Scope

Phase 3 adds the first real C memory-addressing subset on top of the existing Phase 2 integer pipeline:

- one-dimensional fixed arrays
- constrained data pointers
- address-of / dereference
- array and pointer indexing
- indirect data-memory loads and stores

The backend remains shared across `PIC16F628A` and `PIC16F877A`.

## Pointer Model

- pointer size: 16 bits
- storage order: little-endian (`low byte`, then `high byte`)
- address space: PIC16 data memory only
- supported pointer targets:
  - `char`
  - `unsigned char`
  - `int`
  - `unsigned int`
- unsupported:
  - code pointers
  - function pointers
  - pointer-to-pointer
  - pointers to arrays
- null pointer: `0x0000`

In this phase the compiler treats pointers as data-space addresses. The current targets only need the low byte plus the `STATUS.IRP` selector bit for the supported device descriptors and examples, but the pointer value is still carried as a full 16-bit ABI value for consistency.

## Arrays

- supported arrays are fixed-size, one-dimensional only
- supported element types:
  - `char`
  - `unsigned char`
  - `int`
  - `unsigned int`
- declaration bounds must currently be positive integer literals
- global arrays occupy contiguous bytes in the allocated global RAM region
- local arrays occupy contiguous bytes in the current function's static slot region
- there is still no software stack

Array values do not exist as first-class runtime values. Value contexts lower arrays through explicit decay to pointers.

## Direct vs Indirect Access

Named objects use direct banked file-register instructions:

- `STATUS.RP0/RP1` select the direct bank
- globals, locals, params, and temps all lower through direct slots when addressed by name

Pointer-based accesses use indirect addressing:

1. load the pointer low byte into `FSR`
2. derive `STATUS.IRP` from the pointer high byte
3. read or write through `INDF`

The backend does not maintain per-device ad hoc pointer logic. Device differences remain in descriptors.

## Indexing Lowering

Element scaling is explicit:

- `char` / `unsigned char`: element size `1`
- `int` / `unsigned int`: element size `2`

Indexing lowers as:

1. decay array base if needed
2. coerce the index to a 16-bit integer
3. scale the index by element width
4. perform pointer `+` or `-`
5. issue indirect byte loads/stores

16-bit element access becomes two byte-wise accesses at `ptr` and `ptr + 1`.

## ABI Notes

- pointer arguments use the same helper slots as 16-bit integers
- pointer returns use the same `W` + `return_high` convention as 16-bit integers
- boolean compare results remain normalized `0` or `1`
- pointer equality and inequality compare the full 16-bit data-space address

## Current Limits

- no pointer subtraction between two pointers
- no relational pointer comparisons besides `==` / `!=`
- no array initializers
- no stack-based arrays
- no pointer-compatible qualifier conversions beyond exact matching pointee types and literal zero as null
<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
