<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Simulator Workflow

Phase 20 adds `pic16-sim` for running generated HEX files during development.

## Compile A Simulator Example

```bash
mkdir -p build
cargo build --release
./target/release/picc --target pic16f877a -I include --map --list-file -o build/arithmetic.hex examples/sim/arithmetic_sim.c
```

This produces:

- `build/arithmetic.hex`
- `build/arithmetic.map`
- `build/arithmetic.lst`

## Run Until `__halt`

`picc` emits a generated `__halt` loop after `main` returns. Use it as the normal simulator stop symbol:

```bash
./target/release/pic16-sim build/arithmetic.hex --map build/arithmetic.map --run-until __halt --print-symbol result
```

Expected output:

```text
result = 5 (0x05)
```

## Trace Execution

```bash
./target/release/pic16-sim build/arithmetic.hex --map build/arithmetic.map --run-until __halt --trace-file build/arithmetic.trace
```

Use `--trace` instead of `--trace-file` to print trace lines to stdout.

## Inspect State

```bash
./target/release/pic16-sim build/arithmetic.hex --map build/arithmetic.map --run-until __halt --print-regs --print-symbol result
```

`--print-regs` shows PC, W, STATUS, PCLATH, FSR, step count, and return-stack top.

`--print-symbol` reads one byte from RAM/SFR space.

## Max-Step Runs

If no stop symbol is provided, `pic16-sim` runs until the max-step budget:

```bash
./target/release/pic16-sim build/arithmetic.hex --max-steps 1000
```

Use this for firmware that is intended to loop forever and does not have a useful stop symbol.

## Simulator Examples

Current simulator-focused examples:

- `examples/sim/arithmetic_sim.c`
- `examples/sim/function_pointer_sim.c`
- `examples/sim/rom_read_sim.c`

All compute a deterministic global `result` and can be run to `__halt`.

## Limits

`pic16-sim` validates generated PIC16 machine code. It does not replace hardware validation.

Current limits:

- no cycle timing
- no full peripheral model
- no asynchronous interrupt scheduling
- only emitted backend instruction subset is supported
- one-byte symbol printing
