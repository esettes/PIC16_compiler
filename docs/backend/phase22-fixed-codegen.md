<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Phase 22 Fixed Codegen

Fixed-point codegen is raw-storage based:

- Q8.8/UQ8.8 use existing 16-bit load/store/argument/return paths.
- Q16.16/UQ16.16 use existing Phase 21 32-bit load/store/argument/return paths.
- Struct, union, array, and pointer layout use fixed raw byte widths.

Add/subtract are ordinary multi-byte byte-wise operations. Comparisons are lowered as raw integer comparisons before backend branch emission.

Q8.8 multiply/divide use 32-bit raw intermediates and therefore call the existing Phase 21 32-bit helper paths when not optimized away:

- signed Q8.8 uses signed 32-bit multiply/divide helpers
- unsigned UQ8.8 uses unsigned 32-bit multiply/divide helpers

Phase 23 folds Q16.16 multiply/divide when both operands are compile-time constants. Dynamic Q16.16 multiply/divide are still rejected before backend emission until the helper body is split into page-safe PIC14 chunks.

Phase 23 fixed ROM tables lower through little-endian ROM byte payloads:

- Q8.8/UQ8.8 table reads use `RomRead16`
- Q16.16/UQ16.16 table reads use `RomRead32`

ISR code may use fixed loads, stores, add/subtract, casts, and comparisons when inline-safe. Helper-backed fixed multiply/divide remain rejected inside ISR code.
