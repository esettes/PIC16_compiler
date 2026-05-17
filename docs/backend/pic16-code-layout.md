<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# PIC16 Code Layout

`pic16cc` targets classic 14-bit midrange PIC16 program memory.

## Layout Shape

The backend emits:

- reset vector at `0x0000`
- interrupt vector at `0x0004`
- startup and user functions
- runtime helpers only when used
- function-pointer dispatchers only when address-taken functions require them
- RETLW ROM tables for `const __rom` data
- stack overflow trap when `--stack-check` is enabled
- config word at the target descriptor's config address

Phase 25 checks resource fit. Phase 26 checks final HEX validity. Phase 31 checks page-safe control flow.

## Pages and `PCLATH`

Midrange PIC16 `goto` / `call` targets combine instruction operand bits with `PCLATH<4:3>`.

Backend policy:

- before a non-local `goto`, set `PCLATH` for the target label
- before a `call`, set `PCLATH` for the callee label
- after a `call`, restore `PCLATH` to the local continuation when the next local branch depends on it
- conditional branches must be emitted through page-safe helpers, not ad-hoc raw skip-plus-goto sequences

## Debugging Page Bugs

Use:

```bash
picc --target pic16f877a --map --list-file --memory-report -o build/app.hex app.c
```

Then inspect:

- `.map` symbol pages with `page=...`
- `.lst` `setpage` comments
- memory report largest contributors
- simulator traces around the failing PC

Typical risky contributors are large Q16.16 helpers, float helpers, function-pointer dispatchers, and ROM RETLW tables.
