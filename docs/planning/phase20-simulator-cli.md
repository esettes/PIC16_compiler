Muy bien. **Phase 19 es una fase importantísima** porque ya no dependes solo de `.lst/.map/.hex`: ahora tienes tests de ejecución real sobre el código generado. Eso cambia mucho la confianza del proyecto.

Ahora haría una fase pequeña pero muy útil antes de meter `float`:

# Phase 20 — simulador usable: CLI, trazas y depuración

Ahora mismo el emulador existe, pero es **interno de tests**. La siguiente fase debería convertirlo en una herramienta útil para ti y para usuarios/desarrolladores:

```bash
picc --simulate --target pic16f877a -I include src/main.c
```

o una herramienta separada:

```bash
pic16-sim build/main.hex --map build/main.map --trace
```

Yo elegiría **herramienta separada `pic16-sim`**, porque mantiene `picc` como compilador limpio.

Pásale esto a la otra IA:

````text
Continue from the completed Phase 19 repository state of pic16cc.

Do not start float yet.
Do not start recursion yet.
Do not start advanced optimization yet.

The next phase is:

# Phase 20: user-facing simulator CLI, execution traces, and debugging workflow

## Goal

Turn the internal Phase 19 PIC16 execution emulator into a usable developer tool.

Phase 19 added a test-only emulator that can execute generated HEX and validate runtime behavior. Phase 20 should expose this capability through a clean CLI and improve debugging visibility.

Implement:

1. a `pic16-sim` CLI binary
2. HEX loading from user-provided files
3. MAP loading for symbol lookup
4. run-until-symbol support
5. max-instruction limit
6. RAM/SFR inspection
7. optional instruction tracing
8. better simulator diagnostics
9. documentation and examples

This is not a language expansion phase.

## Critical constraints

- Do not redesign the compiler.
- Do not alter normal `picc` compilation behavior.
- Preserve all Phase 2–19 functionality.
- The simulator must remain optional tooling.
- Do not require simulator usage for ordinary compilation.
- Do not add float or recursion in this phase.

## 1. Add simulator CLI binary

Add a new binary:

```text
pic16-sim
````

Cargo release should produce:

```text
target/release/picc
target/release/pic16-sim
```

The simulator CLI should support:

```bash
pic16-sim program.hex
```

Recommended options:

```bash
pic16-sim program.hex \
  --map program.map \
  --run-until __halt \
  --max-steps 200000 \
  --print-symbol result
```

Required options:

* `--map <file>`
* `--run-until <symbol>`
* `--max-steps <n>`
* `--print-symbol <name>`
* `--print-regs`
* `--trace`
* `--trace-file <path>`
* `--help`
* `--version`

## 2. Symbol/map support

The simulator should be able to read `.map` files produced by `picc`.

Required:

* resolve RAM symbols
* resolve program labels where possible
* resolve special symbols:

  * `__halt`
  * `__stack_overflow_trap`
  * globals
  * static data where visible

Example:

```bash
pic16-sim build/test.hex --map build/test.map --run-until __halt --print-symbol result
```

Expected output:

```text
result = 5 (0x05)
```

## 3. Stop conditions

Support:

1. stop after reaching a symbol:

```bash
--run-until __halt
```

2. stop after max steps:

```bash
--max-steps 200000
```

3. stop on simulator error:

* unsupported instruction
* return stack underflow
* PC out of range
* invalid memory access if detectable

If no stop condition is provided, use a safe default max-step limit and document it.

## 4. Register and memory inspection

Support:

```bash
--print-regs
--print-symbol result
--print-symbol some_global
```

Optional but useful:

```bash
--dump-ram
--dump-range 0x20:0x40
```

At minimum, implement:

* register dump
* symbol dump

Register dump should include:

* PC
* W
* STATUS
* PCLATH
* FSR
* top of hardware return stack if useful

## 5. Instruction tracing

Add optional tracing:

```bash
pic16-sim program.hex --map program.map --trace
```

Trace should show at least:

```text
step
PC
instruction word
decoded instruction
W
STATUS
```

If `--trace-file <path>` is provided, write trace to file.

Keep trace compact and readable.

Example:

```text
000012 PC=0008 MOVLW 0x05 W=00 STATUS=...
000013 PC=0009 MOVWF 0x20 W=05 STATUS=...
```

## 6. Simulator errors

Improve simulator errors.

Clear diagnostics for:

* unsupported instruction
* unknown symbol
* invalid map file
* max steps exceeded
* return stack underflow
* hardware stack overflow if modeled
* invalid HEX record if applicable

Do not panic on malformed input. Return user-friendly errors.

## 7. Integration with examples

Add documented workflow:

```bash
picc --target pic16f877a -I include --map --list-file -o build/test.hex examples/pic16f877a/sim_example.c
pic16-sim build/test.hex --map build/test.map --run-until __halt --print-symbol result
```

If current user programs do not generate `__halt`, either:

* document how tests use halt symbols
* add a simulator-friendly example
* or support max-step-based workflows

Do not force normal firmware examples to halt artificially unless they are explicitly simulator examples.

## 8. Examples required

Add examples:

```text
examples/sim/arithmetic_sim.c
examples/sim/function_pointer_sim.c
examples/sim/rom_read_sim.c
```

Each should:

* compute a deterministic result
* store it in a global symbol
* include a halt strategy compatible with `pic16-sim`

If a dedicated `__test_halt()` intrinsic is needed, implement it carefully and document it as simulator/test-only.

## 9. Optional test-only halt intrinsic

If useful, add:

```c
__test_halt();
```

or:

```c
__builtin_halt();
```

Rules:

* only enabled behind an explicit CLI flag, for example `--enable-test-intrinsics`
* not available by default in normal firmware mode
* lowers to a recognizable `__halt` label or safe infinite loop
* documented clearly as simulator/testing support

If this is too much, use max-step stop and symbol-based stop only.

## 10. Tests required

Add tests for:

### CLI

* `pic16-sim --help`
* `pic16-sim --version`
* loading valid HEX
* malformed HEX diagnostic
* missing map diagnostic
* unknown symbol diagnostic

### Execution

* run until symbol
* max-steps stop
* print register state
* print RAM symbol
* trace stdout
* trace file output

### Integration

Compile a small program with `picc`, then run it with `pic16-sim`, and assert printed result.

Test at least:

* arithmetic result
* function pointer result
* ROM read result

### Regression

All Phase 2 through Phase 19 tests must still pass.

## 11. Documentation

Update:

* README.md
* DESIGN.md
* docs/architecture/overview.md
* docs/developer-guide/testing.md
* docs/sim/pic16-core-emulator.md

Add:

* docs/sim/pic16-sim-cli.md
* docs/developer-guide/simulator-workflow.md

Document:

* how to build `pic16-sim`
* how to run HEX files
* how to use `.map` symbols
* how to trace instructions
* how to inspect registers and symbols
* simulator limitations
* difference between test emulator and real hardware
* unsupported peripherals

## 12. Acceptance criteria

Phase 20 is complete only if:

1. `cargo check` passes
2. `cargo test` passes
3. `cargo clippy --all-targets -- -D warnings` passes
4. `cargo build --release` produces both `picc` and `pic16-sim`
5. `pic16-sim --help` works
6. `pic16-sim --version` works
7. `pic16-sim` can load a generated HEX file
8. `pic16-sim` can resolve symbols from a generated MAP file
9. `pic16-sim` can run until a symbol or max-step limit
10. `pic16-sim` can print register state and symbol values
11. tracing works
12. simulator errors are user-friendly
13. previous Phase 2–19 tests still pass
14. docs accurately describe usage and limitations

## Output required

Return:

1. Phase 20 summary
2. simulator CLI design
3. command examples
4. map/symbol support
5. stop-condition behavior
6. register/memory inspection behavior
7. trace behavior
8. errors/diagnostics added
9. examples added
10. tests added
11. docs updated
12. remaining limitations
13. validation:

* cargo check
* cargo test
* cargo clippy --all-targets -- -D warnings
* cargo build --release
