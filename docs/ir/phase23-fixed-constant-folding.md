<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Phase 23 Fixed Constant Folding

Phase 23 keeps fixed-point IR values as raw integer-width scalars:

- Q8.8/UQ8.8 use 16-bit raw temps
- Q16.16/UQ16.16 use 32-bit raw temps
- fixed ROM reads lower to `RomRead16` or `RomRead32`

Fixed decimal literals enter semantic analysis as raw `IntLiteral` values carrying a fixed scalar type. Constant folding then reuses the fixed evaluator so runtime and compile-time raw values match.

Folded operations:

- fixed decimal literal raw conversion
- Q8.8 add/sub/mul/div constants
- Q16.16 add/sub/mul/div constants
- fixed comparisons
- integer-to-fixed casts
- fixed-to-integer casts
- fixed-to-fixed casts

Division by constant fixed zero is diagnosed before lowering.

Phase 24 adds dynamic Q16.16/UQ16.16 helper lowering for non-constant operands. Constant folding still runs first, so compile-time expressions do not emit helper calls.
