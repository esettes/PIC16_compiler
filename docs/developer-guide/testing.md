<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Testing Guide

Phase 31 backend page-safety validation lives in `src/backend/pic16/midrange14/encoder.rs`, `tests/compiler_pipeline.rs`, and `tests/execution_sim.rs`. It checks unsafe cross-page `goto` / `call` rejection, map/listing page metadata, Q16.16 helper execution across multi-page layouts, float/ROM/function-pointer page paths, and stack-check trap layout.

Phase 32 layout validation lives in the same backend and integration suites. It checks same-page `setpage` relaxation, cross-page `setpage` preservation, page-layout memory report output, map `Code Layout`, and simulator execution with float/fixed ROM tables plus large helper paths.

Phase 33 runtime-cost validation lives in `src/backend/pic16/midrange14/runtime.rs`, `tests/compiler_pipeline.rs`, and `tests/execution_sim.rs`. It checks helper catalog metadata, dependency graph validation, runtime profile CLI parsing, helper cost reports, map grouping, unused-helper pruning, checked-in runtime-size examples, and simulator execution for integer plus Q16.16 helper paths.

Phase 34 runtime-compaction validation lives in `tests/compiler_pipeline.rs` and `tests/execution_sim.rs`. It checks that `--runtime-profile small` selects `__rt_u32_divmod_core`, reports `variant=small` and helper dependencies, reduces representative unsigned 32-bit div/mod output versus `balanced`, keeps stack-report helper accounting visible, compiles checked-in profile examples, and executes unsigned/signed 32-bit div/mod wrappers through the simulator.

Phase 26 validation lives in `tests/compiler_pipeline.rs` and `src/hex/intel_hex.rs`. It checks raw and symbolic config words, duplicate/unknown config diagnostics, config emission in `.hex/.map/.lst`, final HEX validation reports, program overflow rejection without `--size`, programmer command printing, configurable Makefile flashing, and hardware smoke examples.

Phase 25 resource-report validation lives in `tests/compiler_pipeline.rs`. It checks target descriptors, `--size`, `--memory-report`, `--memory-report-file`, map/listing resource summaries, helper contribution reporting, ROM-table contribution reporting, data-RAM overflow diagnostics, and stack-region overflow diagnostics.

Phase 24 runtime validation lives in `tests/execution_sim.rs`. It compiles C programs to HEX, runs them through the simulator, and checks fixed-point RAM results for decimal literals, fixed ROM table reads, fixed casts, Q8.8 arithmetic, UQ8.8 arithmetic, Q16.16 add/subtract/compare, Q16.16 constant-folded multiply/divide, and dynamic Q16.16/UQ16.16 helper-backed multiply/divide.

Phase 24 diagnostics and artifacts are covered in `tests/compiler_pipeline.rs` for malformed/unsupported/out-of-range fixed literals, implicit fixed narrowing, bitwise/modulo fixed rejection, fixed division by constant zero, ISR helper restrictions, fixed ROM objects, dynamic Q16.16 helper map/listing visibility, stack-report helper accounting, and checked-in fixed examples.

Phase 21 runtime and diagnostic coverage remains in the same files for 32-bit integer behavior.

`pic16cc` currently uses three testing layers:

- Rust unit tests for frontend, IR, backend, encoder, and simulator internals
- compiler pipeline integration tests that validate diagnostics plus `.hex` / `.map` / `.lst` / `.asm` shape
- Phase 19 execution tests that run generated PIC16 machine code inside the internal emulator
- Phase 20 simulator CLI tests that run `pic16-sim` as a user-facing binary

Core commands:

```bash
cargo check
cargo test
cargo test --test execution_sim
cargo test --test sim_cli
cargo clippy --all-targets -- -D warnings
```

## Phase 26 Hardware Workflow Tests

No test requires actual hardware. Tests validate:

- generated HEX has valid checksum and EOF
- config word lands at the target config address
- external programmer command shape is configurable
- example Makefiles expose `FLASH_CMD` and `FLASH_ARGS`
- hardware smoke examples compile with `--verify-hex`

## Memory Report Tests

Use compiler-pipeline tests for:

- `--size`
- `--memory-report`
- `--memory-report-file`
- `.map` `Memory Summary`
- `.lst` resource summary comments
- helper-heavy programs showing `__rt_mul_q16_16` / `__rt_div_q16_16`
- ROM-table programs showing ROM table contribution
- artificial large globals causing data-RAM overflow
- local/argument pressure causing stack-region overflow

## Simulator CLI Tests

Phase 20 CLI validation lives in:

- `src/sim_cli.rs`
- `src/bin/pic16-sim.rs`
- `tests/sim_cli.rs`

The pattern is:

1. compile a small C source to Intel HEX and `.map`
2. invoke `pic16-sim` with `--map`
3. run until `__halt` or a max-step budget
4. assert stdout/stderr for symbols, registers, traces, and diagnostics

Use simulator CLI tests for:

- `--help` / `--version`
- malformed HEX diagnostics
- missing map diagnostics
- unknown symbol diagnostics
- `--run-until`
- `--max-steps`
- `--print-regs`
- `--print-symbol`
- `--trace`
- `--trace-file`

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
- fixed-point scaling and helper-backed Q8.8 arithmetic
- finite float literals, float helper paths, Phase 28/29 dynamic cast paths, dynamic float comparisons, Phase 30 ROM float reads, and float ABI behavior
- stack-first calls and nested calls
- pointer dereference and `FSR/INDF`
- function-pointer dispatch
- aggregate layout and access
- ROM table reads, including integer, fixed-point, and float payloads

## Adding a New Execution Test

Keep fixtures small and deterministic.

- prefer a single `result` symbol or one small struct/array result region
- make `main` finish through the normal generated `__halt` path
- use constant expected values instead of relying on timing or peripherals
- use `pic16f628a` unless the scenario needs the larger `pic16f877a`
- use `pic16f877a` for float helper cases; generic and cast float helpers are large and resource fitting may reject helper-heavy fixtures
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
- `pic16-sim` validates generated machine-code behavior; it is not a hardware-accurate peripheral simulator
