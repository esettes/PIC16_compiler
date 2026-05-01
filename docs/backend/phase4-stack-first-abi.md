<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Phase 4 Stack-first ABI

This document describes current call ABI used by the `picc` CLI compiler (`pic16cc` crate).

## Status

Phase 4 ABI is active.

Phase status:

- retained as the active ABI foundation for Phase 6
- frozen for stabilization in this branch

- stack-first
- caller-pushed
- caller-cleanup
- upward-growing software stack
- no fallback to `arg0` / `arg1`

Historical Phase 2/3 helper-slot ABI docs remain archival only.

## Scalar Layout

- `char`: 8-bit signed
- `unsigned char`: 8-bit unsigned
- `int`: 16-bit signed
- `unsigned int`: 16-bit unsigned
- pointer size: 16 bits
- pointer representation: little-endian PIC16 data-space address
- 16-bit byte order everywhere: low byte first, high byte second

## Argument Passing

All arguments pass through software stack.

Rules:

- evaluation order follows current IR lowering order
- caller pushes arguments left-to-right
- each argument pushes low byte first, then high byte when width is 16 bits
- callee parameters appear contiguously from `FP + 0`

Examples:

- 8-bit arg: 1 byte
- 16-bit arg: low then high
- pointer arg: low then high
- 3+ args: contiguous, no special slots

## Return Values

- 8-bit return: `W`
- 16-bit return: low byte in `W`, high byte in helper slot `return_high`
- pointer return: same rule as 16-bit integer

## Caller / Callee Contract

Before call:

- `SP = caller_top`

Caller:

1. evaluate arguments
2. push argument bytes left-to-right
3. `CALL`
4. subtract callee argument bytes from `SP`
5. read return value from `W` / `return_high`

Callee:

1. save caller `FP`
2. set `FP = SP - arg_bytes`
3. reserve locals + IR temps above saved `FP`
4. execute body
5. set `SP = FP + arg_bytes`
6. restore caller `FP`
7. `RETURN`

Only caller cleans argument bytes. Callee never subtracts caller argument area.

## Frame-relative Parameters

Given `arg_bytes = A`:

- parameter bytes live at `FP + 0 .. A - 1`
- saved caller `FP` lives at `FP + A .. A + 1`
- locals and IR temps begin at `FP + A + 2`

## Notes

- `main` still requires zero parameters
- recursion is rejected because stack depth is computed statically and there is no runtime overflow check
- backend map output exposes helper symbols such as `__abi.stack_ptr.lo` and stack bounds such as `__stack.base`
<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
