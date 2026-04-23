# Phase 2 ABI and Compare Lowering

## Scope

This document describes the Phase 2 ABI and backend behavior for 16-bit integers on the shared PIC16 `midrange14` backend.

Supported devices in this phase:

- `PIC16F628A`
- `PIC16F877A`

## 16-bit Representation

- representation: two bytes in RAM
- byte order: little-endian
- low byte address: `base`
- high byte address: `base + 1`

This layout is used consistently for:

- globals
- locals
- parameters
- temporaries
- helper slots
- return values after call lowering

## ABI Slots

Phase 2 keeps a fixed-slot ABI. There is no software stack yet.

Backend-managed helper slots:

- `arg0.lo`
- `arg0.hi`
- `arg1.lo`
- `arg1.hi`
- `return_high`
- `scratch0`
- `scratch1`

Rules:

- at most two parameters
- at most two call arguments
- `main` must not take parameters
- helper slots are internal backend resources, not user-visible symbols

## Argument Passing

- first 8-bit argument: `arg0.lo`
- first 16-bit argument: `arg0.lo` + `arg0.hi`
- second 8-bit argument: `arg1.lo`
- second 16-bit argument: `arg1.lo` + `arg1.hi`

Unused high bytes are cleared for 8-bit arguments so callee-side reads are deterministic.

## Return Values

- 8-bit return: `W`
- 16-bit return:
  - low byte in `W`
  - high byte in `return_high`

Caller copies the return convention into the destination temporary immediately after `CALL`.

## Boolean Results

Boolean-like expressions are normalized to `0` or `1`.

- storage type: `unsigned char`
- false: `0`
- true: `1`

The IR lowers compare expressions through branch form, then writes `0` or `1` into a temp when a materialized value is needed.

## 16-bit Arithmetic

Phase 2 codegen supports:

- add
- subtract
- bitwise and/or/xor
- bitwise not
- unary negate

Lowering model:

- low byte executes first
- carry/borrow propagates into high byte explicitly
- temporaries are stored in allocatable GPR RAM

## Equality and Inequality

8-bit:

- subtract one byte
- branch on `STATUS.Z`

16-bit:

1. compare high byte
2. if high bytes differ, result is known
3. otherwise compare low byte

## Unsigned Relations

Supported:

- `<`
- `<=`
- `>`
- `>=`

Lowering:

1. subtract most-significant byte
2. inspect `STATUS.C` and `STATUS.Z`
3. only compare low byte when high byte is equal

Meaning:

- `C = 1`: no borrow, so `lhs >= rhs`
- `C = 0`: borrow, so `lhs < rhs`
- `Z = 1`: compared bytes were equal

## Signed Relations

Supported:

- `<`
- `<=`
- `>`
- `>=`

Lowering:

1. load sign byte of both operands
2. xor sign bytes
3. if signs differ, decide from sign bit only
4. if signs match, reuse unsigned compare lowering

Reason:

- same-sign signed ordering matches unsigned ordering
- different-sign case can be resolved from sign bit without full subtract-with-overflow machinery

## Banking and Paging

The backend remains explicit about PIC16 control state:

- RAM banking through `STATUS.RP0/RP1`
- code paging through `PCLATH<4:3>` before `CALL` and `GOTO`

No generic 8-bit CPU shortcutting is used here.

## Deferred ABI Work

- software stack
- more than two parameters / arguments
- recursive calls
- ISR calling convention
- runtime helpers for multiply/divide/modulo
- implicit equal-width mixed signedness compare lowering
