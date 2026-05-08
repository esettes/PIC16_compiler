<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Phase 19 Emulator Validation

Phase 19 adds execution validation for generated PIC16 code.

Before this phase, most regression coverage stopped at:

- compile success/failure
- diagnostics
- `.hex` / `.map` / `.lst` / `.asm` shape

That catches many bugs, but not runtime mistakes such as:

- wrong stack-frame byte offsets
- `W` clobber around calls or epilogues
- broken indirect load/store through `FSR/INDF`
- bad function-pointer dispatcher state
- ROM read path regressions

## Chosen Approach

Phase 19 uses **Option A**: a small internal emulator.

Properties:

- deterministic and self-contained for CI
- test-only in this phase
- consumes generated Intel HEX, not a special alternate IR path
- modular: compiler pipeline and CLI stay unchanged for normal users

## Test Flow

Execution tests in `tests/execution_sim.rs` do:

1. compile source to HEX
2. load HEX with `ProgramImage::from_hex_file`
3. create `Pic16Core` for the selected target
4. run until a map symbol such as `__halt`
5. inspect final RAM/SFR state

Stop strategy:

- primary stop label: `__halt`
- alternate stop label when useful: `__stack_overflow_trap`
- hard step ceiling: `200_000` instructions

## Runtime Coverage

Phase 19 runtime tests cover:

- 8-bit and 16-bit arithmetic
- multiply/divide/modulo helpers
- `if` / `while` / `for` / `do while`
- `switch` / `case` / `default`
- stack-first calls with 3+ arguments
- nested calls
- stack-check-enabled call paths
- function-pointer dispatch
- globals, locals, static locals
- structs, unions, bitfields
- multidimensional arrays
- pointers and pointer-to-pointer
- constant and dynamic ROM byte reads
- 16-bit ROM table reads

ISR timing is still deferred, but `retfie` execution itself is validated in simulator unit tests.

## Limits

- no full peripheral/timer model
- no asynchronous interrupt scheduling
- no cycle-accurate timing claim
- only the currently emitted backend instruction subset is supported
- unsupported instruction words fail clearly
