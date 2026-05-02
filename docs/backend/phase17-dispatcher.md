<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Phase 17 Dispatcher

Phase 17 implements function-pointer calls with generated dispatch-ID trampolines instead of raw computed PIC16 calls.

## Representation

- function-pointer value is a 16-bit dispatch ID
- `0` is null
- real targets start at `1`
- IDs are grouped by normalized function-pointer signature

## Codegen

- backend emits one dispatcher label per signature
- caller evaluates and pushes arguments with the normal stack-first ABI
- caller stores the 16-bit dispatch ID into backend scratch slots
- caller then performs one ordinary direct `call` to the signature dispatcher
- dispatcher checks the high byte is zero, compares the low byte against known IDs, then does one ordinary page-safe direct call to the real function

## Miss Path

- invalid dynamic IDs never jump to random code
- dispatcher miss path returns with zeroed return registers

## Restrictions

- indirect calls remain forbidden in ISR
- no raw computed `CALL`
- no ROM function-pointer tables
