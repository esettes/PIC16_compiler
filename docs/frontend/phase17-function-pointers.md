<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Phase 17 Function Pointers

Phase 17 adds one conservative function-pointer model for classic 14-bit PIC16.

## Supported Syntax

- `void (*handler)(void);`
- `typedef void (*Handler)(void);`
- arrays of supported function-pointer types
- struct fields of supported function-pointer types

## Supported Signatures

- `void (*)(void)`
- `void (*)(unsigned char)`
- `unsigned char (*)(void)`
- `unsigned char (*)(unsigned char)`
- `int (*)(int)`
- `unsigned int (*)(unsigned int)`

More generally, Phase 17 accepts zero or one integer parameter and `void`/8-bit/16-bit integer returns.

## Semantics

- taking the address of one supported function yields a function-pointer value
- using one function name in value context yields the same function-pointer value
- null function-pointer value is literal `0`
- equality/inequality with compatible function-pointer types is supported
- compatible function-pointer calls use the normal stack-first ABI

## Representation

- function-pointer values are 16-bit dispatch IDs
- they are not raw PIC16 code addresses
- each supported signature gets one generated dispatcher chain

## Restrictions

- pointer-to-function-pointer objects are rejected
- arithmetic on function pointers is rejected
- relational comparisons on function pointers are rejected
- data-pointer/function-pointer mixing is rejected
- function-pointer calls inside ISR are rejected
- ROM function-pointer tables are rejected
