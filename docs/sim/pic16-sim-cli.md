<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# `pic16-sim` CLI

`pic16-sim` is the Phase 20 user-facing wrapper around the internal PIC16 emulator.

It runs Intel HEX files produced by `picc` and optionally uses `.map` files for symbol lookup.

## Build

```bash
cargo build --release
./target/release/pic16-sim --version
./target/release/pic16-sim --help
```

Release builds produce both binaries:

```text
target/release/picc
target/release/pic16-sim
```

## Basic Run

```bash
pic16-sim program.hex
```

Without `--run-until`, the simulator runs until the max-step budget is reached. The default budget is `200000` instructions.

## Run Until Symbol

```bash
pic16-sim program.hex --map program.map --run-until __halt
```

`--run-until` resolves code symbols from the `.map` file and stops before executing the target PC.

Common stop labels:

- `__halt`
- `__stack_overflow_trap`

## Symbol Inspection

```bash
pic16-sim program.hex --map program.map --run-until __halt --print-symbol result
```

Example output:

```text
result = 5 (0x05)
```

Symbol printing currently reads one byte from RAM/SFR space. It supports:

- data symbols from `Data Symbols`
- device SFR names such as `PORTA`, `PORTB`, `STATUS`, and `PCLATH`

## Register Inspection

```bash
pic16-sim program.hex --map program.map --run-until __halt --print-regs
```

Register output includes:

- PC
- W
- STATUS
- PCLATH
- FSR
- executed step count
- top hardware return-stack entry, when present

## Tracing

Trace to stdout:

```bash
pic16-sim program.hex --map program.map --run-until __halt --trace
```

Trace to file:

```bash
pic16-sim program.hex --map program.map --run-until __halt --trace-file build/trace.txt
```

Trace lines include:

```text
000012 PC=0008 WORD=3005 MOVLW 0x05      W=00 STATUS=18
```

Fields:

- step
- PC
- encoded instruction word
- decoded instruction
- W
- STATUS

## Target Selection

Default target is `pic16f877a`.

Use `--target` to select another supported device:

```bash
pic16-sim --target pic16f628a program.hex --map program.map --run-until __halt
```

## Diagnostics

User-facing errors include:

- malformed Intel HEX input
- missing or invalid `.map` file
- unknown symbol
- unsupported instruction word
- missing instruction at current PC
- return-stack underflow
- step-limit exhaustion before a requested stop symbol

## Limits

`pic16-sim` is not a complete hardware simulator.

Current limits:

- no cycle timing
- no full peripheral timing or side effects
- no asynchronous interrupt scheduling
- only the instruction subset emitted by the backend is implemented
- symbol printing is byte-oriented
