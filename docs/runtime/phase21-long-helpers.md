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
