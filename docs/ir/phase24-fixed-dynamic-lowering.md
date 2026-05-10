<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Phase 24 Fixed Dynamic Lowering

Phase 24 removes the Phase 23 semantic deferral for dynamic Q16.16 and UQ16.16 multiply/divide.

Lowering rules:

- constant Q16.16/UQ16.16 multiply/divide still fold before IR emission
- dynamic `*` and `/` over Q16.16/UQ16.16 remain typed binary IR operations
- backend helper selection maps those operations to `__rt_mul_q16_16`, `__rt_mul_uq16_16`, `__rt_div_q16_16`, or `__rt_div_uq16_16`
- raw value width stays 32-bit; no 64-bit public IR or C type is added

ISR policy:

- constant-folded Q16.16 expressions can be used when they lower inline
- dynamic Q16.16 multiply/divide remain rejected inside ISR code because they require runtime helper calls

Runtime validation:

- simulator tests compile C to HEX, execute with the PIC16 simulator, and assert RAM results for signed/unsigned multiply, signed/unsigned divide, division by zero, function argument/return, struct field operations, and array element operations
