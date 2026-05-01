<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Phase 15 Bitfield and Union Codegen

Phase 15 keeps backend layering unchanged.

## Union Codegen

- union objects use one contiguous storage block sized to the maximum field size
- field access reuses the normal aggregate address plus constant-offset machinery
- whole-union copy reuses the same byte-wise indirect load/store path as whole-struct copy

## Bitfield Codegen

Bitfield reads and writes lower through ordinary PIC16 instructions generated from existing IR:

- loads from RAM or frame storage
- masks with `andlw` / `andwf`
- shifts through ordinary integer lowering
- read-modify-write updates through mask clear plus `iorlw` / `iorwf` style composition

One-bit cases may incidentally fold into `bsf` / `bcf` when the existing backend instruction selection can prove it, but Phase 15 does not require a dedicated special case.

## Current Restrictions

- bitfield storage is RAM-only in this phase
- ROM union objects and ROM bitfield objects are rejected
- whole-aggregate copy inside ISR remains rejected before backend lowering
- no anonymous union support is added in the backend
