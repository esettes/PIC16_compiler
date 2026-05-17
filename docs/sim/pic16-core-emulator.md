<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# PIC16 Core Emulator

Phase 19 introduced a small internal PIC16 emulator in `src/sim/mod.rs`.

Its job is narrow:

- execute the subset of 14-bit PIC16 instructions emitted by `pic16cc`
- validate runtime behavior of generated HEX in tests
- support the optional Phase 20 `pic16-sim` CLI
- fail clearly when the backend emits an unsupported instruction

It is **not** a full PIC16 device simulator.

## Modeled CPU State

Current model includes:

- program counter
- `W`
- `STATUS` bits used by generated code: `C`, `DC`, `Z`, `IRP`
- `PCLATH`
- `FSR` / `INDF`
- 512-byte banked data RAM view with common/shared-address canonicalization
- hardware return stack for `call` / `return` / `retlw` / `retfie`

Program memory is loaded from Intel HEX records into 14-bit words.

## Supported Instructions

Current decoder/executor supports:

- `nop`
- `movlw`
- `movwf`
- `movf`
- `clrf`
- `clrw`
- `addlw`
- `andlw`
- `iorlw`
- `xorlw`
- `addwf`
- `andwf`
- `iorwf`
- `xorwf`
- `subwf`
- `rlf`
- `rrf`
- `swapf`
- `bcf`
- `bsf`
- `btfsc`
- `btfss`
- `goto`
- `call`
- `retlw`
- `return`
- `retfie`

Backend pseudo-ops such as `setpage` and `setpclpage` are not simulator instructions; they are expanded before HEX emission.

## RAM and SFR Model

RAM model is intentionally simple:

- file registers and SFRs share one byte-addressable array
- common registers and shared window addresses canonicalize to the expected low-bank view
- `INDF` dereferences through `FSR` plus `STATUS.IRP`
- `PCL` writes rebuild the next PC using `PCLATH`

For execution tests, SFRs like `PORTA`, `PORTB`, and `TRIS*` behave as ordinary registers unless a test asserts on them directly.

## Calls, Paging, and Stop Conditions

- `call` pushes `PC + 1` onto the hardware return stack
- `goto` / `call` combine their encoded target with `PCLATH<4:3>`
- `retlw` sets `W` and pops the return stack
- `retfie` currently behaves like a return-stack pop for execution-validation purposes
- Phase 31 page-safety tests rely on this real `PCLATH` behavior to catch stale-page backend bugs

Test runs normally stop by:

- running until the PC reaches a known map symbol such as `__halt`
- enforcing a maximum step budget to catch runaway code

## Failure Behavior

Simulator errors are explicit:

- malformed HEX input
- missing instruction at current PC
- unsupported opcode word
- return-stack underflow
- step-limit exhaustion

This is intentional. If backend code generation drifts outside the supported subset, tests should fail loudly instead of silently emulating the wrong thing.

## Deferred Scope

- cycle timing
- full peripheral behavior
- asynchronous interrupts
- every legal PIC16 instruction

## User-Facing CLI

Phase 20 adds `pic16-sim`, a thin CLI wrapper around this core.

The CLI owns:

- command-line parsing
- `.map` symbol lookup
- run-until-symbol behavior
- register and symbol printing
- trace rendering
- user-facing diagnostics

The core remains reusable by tests and does not depend on the CLI.
