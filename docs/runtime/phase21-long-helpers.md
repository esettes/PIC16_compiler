<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Phase 21 Long Helpers

Phase 21 adds helper variants for 32-bit arithmetic:

- `__rt_mul_u32`, `__rt_mul_i32`
- `__rt_div_u32`, `__rt_div_i32`
- `__rt_mod_u32`, `__rt_mod_i32`
- `__rt_shl32`, `__rt_shr_u32`, `__rt_shr_i32`

Unsigned multiplication uses shift-add. Division and modulo use restoring division. Signed helpers normalize operands, call the unsigned core, and restore the result sign.

Helpers follow the Stack-first ABI. Two 32-bit operands consume eight argument bytes. 32-bit returns use `W`, `return_high`, `return_upper0`, and `return_upper1`.

ISR code may use inline-safe 32-bit loads, stores, simple comparisons, and constant shifts. Operations requiring helper calls remain rejected inside interrupt handlers.

Phase 22 fixed-point Q8.8 multiply/divide reuses these 32-bit helper paths by widening raw Q8.8 operands to 32-bit, applying the fixed-point scale shift, and truncating back to 16-bit raw storage.

Phase 23 folds Q16.16 multiply/divide constants exactly. Phase 24 adds dedicated page-safe Q16.16/UQ16.16 dynamic helpers rather than reusing these 32-bit integer helpers, because correct fixed-point multiply/divide needs wider intermediate behavior.
