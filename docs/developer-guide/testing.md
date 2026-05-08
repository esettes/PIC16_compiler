<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Testing Guide

`pic16cc` currently uses three testing layers:

- Rust unit tests for frontend, IR, backend, encoder, and simulator internals
- compiler pipeline integration tests that validate diagnostics plus `.hex` / `.map` / `.lst` / `.asm` shape
- Phase 19 execution tests that run generated PIC16 machine code inside the internal emulator

Core commands:

```bash
cargo check
cargo test
cargo test --test execution_sim
cargo clippy --all-targets -- -D warnings
```

## Execution Tests

Phase 19 execution validation lives in:

- `src/sim/mod.rs`
- `tests/execution_sim.rs`

The pattern is:

1. compile a small C source to Intel HEX with ordinary `execute(...)`
2. read the generated map to find result symbols and stop labels
3. load the HEX into `ProgramImage`
4. run `Pic16Core` until `__halt` or another known label
5. assert final RAM/SFR contents

Use execution tests when output shape is not enough to prove behavior, especially for:

- arithmetic helpers
- stack-first calls and nested calls
- pointer dereference and `FSR/INDF`
- function-pointer dispatch
- aggregate layout and access
- ROM table reads

## Adding a New Execution Test

Keep fixtures small and deterministic.

- prefer a single `result` symbol or one small struct/array result region
- make `main` finish through the normal generated `__halt` path
- use constant expected values instead of relying on timing or peripherals
- use `pic16f628a` unless the scenario needs the larger `pic16f877a`
- if a case needs stack checks, compile with `stack_check: true`

Good pattern:

```c
unsigned char result;

void main(void) {
    result = 2 + 3;
}
```

Then assert `result == 5` after simulation.

## Limits

- simulator is core-only, not a full device/peripheral emulator
- asynchronous interrupt timing is not modeled
- unsupported opcodes must fail explicitly instead of being guessed
- execution validation complements, not replaces, map/listing/diagnostic tests
