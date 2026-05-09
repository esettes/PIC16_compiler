<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Phase 21 Long Lowering

The IR keeps integer constants as `i64` and relies on `Type` widths for truncation and signed interpretation. `I32` and `U32` temps use four bytes in stack frames.

Lowering rules:

- 32-bit loads, stores, copies, casts, array indexing, struct fields, and union overlays lower as byte-wise operations.
- 32-bit comparisons lower to high-byte-first branches.
- 32-bit pointer arithmetic scales `long *` indices by 4.
- Pointer subtraction supports element sizes 1, 2, and 4; size 4 divides the raw byte difference with a right shift by 2.
- Cast IR carries source type metadata so constant folding and backend extension/truncation preserve 8/16/32-bit semantics.
