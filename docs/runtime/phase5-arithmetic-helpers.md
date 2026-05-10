<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Phase 5 Arithmetic Helpers

Phase 22 fixed-point Q8.8 multiply/divide uses raw 32-bit intermediate lowering through the Phase 21 helper model; see `docs/runtime/phase22-fixed-helpers.md`.

Phase 21 adds 32-bit helper variants for `long` and `unsigned long`; see `docs/runtime/phase21-long-helpers.md`. Existing 8-bit and 16-bit helper names and calling behavior remain compatible.

Phase status:

- 8-bit and 16-bit helper behavior is frozen under the Phase 6 stabilization baseline
- Phase 21 extends the same helper model to 32-bit integer types
- Phase 22 reuses 32-bit helpers for Q8.8 fixed-point multiply/divide
- Phase 23 folds Q16.16 fixed multiply/divide constants
- Phase 24 adds dedicated dynamic Q16.16/UQ16.16 multiply/divide helpers

Phase 5 adds compiler-generated PIC16 runtime helpers for:

- multiply: `*`
- divide: `/`
- modulo: `%`
- dynamic left shift: `<<`
- dynamic right shift: `>>`

Supported scalar types:

- `char`
- `unsigned char`
- `int`
- `unsigned int`
- `long`
- `unsigned long`
- `__fixed8_8` / `__ufixed8_8` through raw 32-bit lowering for multiply/divide

## Helper Families

Multiply:

- `__rt_mul_u8`
- `__rt_mul_i8`
- `__rt_mul_u16`
- `__rt_mul_i16`

Divide:

- `__rt_div_u8`
- `__rt_div_i8`
- `__rt_div_u16`
- `__rt_div_i16`

Modulo:

- `__rt_mod_u8`
- `__rt_mod_i8`
- `__rt_mod_u16`
- `__rt_mod_i16`

Shifts:

- `__rt_shl8`
- `__rt_shl16`
- `__rt_shr_u8`
- `__rt_shr_i8`
- `__rt_shr_u16`
- `__rt_shr_i16`

Helpers are emitted only when codegen needs them. Their labels appear in `.map` and `.lst`.

## Algorithms

Multiply:

- unsigned multiply uses shift-and-add loops
- multiplicand shifts left in helper arg slot
- multiplier shifts right in helper arg slot
- result accumulates in helper frame local storage

Divide / Modulo:

- unsigned divide/modulo use loop-based restoring division
- dividend arg slot becomes quotient during iteration
- remainder lives in helper frame local storage
- modulo returns remainder

Signed variants:

- normalize negative operands to absolute values
- record sign flags in helper-local flag byte
- run unsigned core
- restore quotient or remainder sign using two's-complement negation

Shifts:

- constant shifts lower inline in caller, not through these helpers
- dynamic shifts use one-bit looped helpers
- helper clamps dynamic shift count to operand bit width before looping

## ABI Integration

Helpers use same repaired stack-first ABI as normal functions:

- caller pushes args left-to-right
- scalar bytes push low byte first
- caller cleans arg bytes after return
- helper saves caller `FP`
- helper restores `SP` / caller `FP` before `return`
- 8-bit result returns in `W`
- 16-bit result returns low byte in `W`, high byte in `return_high`
- 32-bit helper results return through `W`, `return_high`, `return_upper0`, and `return_upper1`

Helpers may mutate their own arg slots as working storage. This is ABI-safe because caller pops helper args after return.

## Behavior

Multiply:

- 8-bit multiply returns low 8 bits
- 16-bit multiply returns low 16 bits
- signed multiply follows two's-complement wrap/truncation

Divide / Modulo:

- division by constant zero is rejected during semantic analysis
- modulo by constant zero is rejected during semantic analysis
- dynamic zero divisor returns `0`
- signed divide/modulo follow two's-complement helper normalization

Shifts:

- unsigned right shift is logical
- signed right shift is arithmetic
- constant shift count `>=` bit width is rejected
- dynamic shift count clamps to operand width
- left shift is fixed-width and truncating

## Remaining Limits

- no runtime trap for dynamic zero divisor
- no full ISO C promotion lattice yet
- no fixed-point modulo helper path
- recursion still rejected globally because stack depth remains static
<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
