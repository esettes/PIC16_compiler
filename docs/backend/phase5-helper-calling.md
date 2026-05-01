<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Phase 5 Helper Calling

This document defines how backend-generated arithmetic helpers integrate with repaired Phase 4 stack ABI.

Phase status:

- helper calling behavior is frozen as part of Phase 6 stabilization
- no new helper ABI variants are introduced in this branch

## Contract

Caller:

- pushes helper args left-to-right
- pushes scalar bytes low byte first
- issues `CALL`
- subtracts helper arg bytes after return

Helper callee:

- computes `FP = SP - arg_bytes`
- saves caller `FP`
- allocates helper locals above saved `FP`
- writes result into normal return locations
- restores `SP`
- restores caller `FP`
- returns

No helper uses a second private ABI.

## Frame Shape

For helper with `arg_bytes = A`:

- `FP + 0 .. A - 1`: mutable helper arg slots
- `FP + A`: saved caller `FP` low
- `FP + A + 1`: saved caller `FP` high
- `FP + A + 2 ..`: helper locals

Typical helper locals:

- result or remainder working storage
- loop count byte
- signed-operation flag byte

## Byte Order

All scalar/pointer args use little-endian byte order:

- 8-bit: one byte
- 16-bit / pointer: low byte, then high byte

Return:

- 8-bit: `W`
- 16-bit / pointer: low byte `W`, high byte `return_high`

## Safety Properties

Because helpers reuse normal ABI:

- caller locals stay intact
- caller IR temps stay intact
- nested user calls around helper calls stay valid
- pointer/array lowering still uses normal `FSR/INDF` paths
- helper labels can be audited in `.asm`, `.lst`, `.map`

## Current Lowering Choices

Inline:

- constant folds
- `x * 0`
- `x * 1`
- `x / 1`
- `x % 1`
- `x << 0`
- `x >> 0`
- constant shifts
- multiply-by-power-of-two when recognized

Helper path:

- most multiply/divide/modulo
- dynamic shifts

Division/modulo diagnostics and runtime behavior:

- constant zero divisors are rejected during semantic analysis
- dynamic zero divisors in helper paths return `0`

## Helper Labels

Backend emits labels named like:

- `__rt_mul_u16`
- `__rt_div_i8`
- `__rt_mod_u16`
- `__rt_shr_i16`

These labels are stable enough for golden asm/map/listing tests in current phase.
<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
