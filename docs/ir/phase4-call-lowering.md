<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Phase 4 Call Lowering

This document describes how direct calls lower from typed tree to IR and then to PIC16 backend.

## Semantic Layer

Semantic analysis:

- validates callee exists and is callable
- validates argument count matches signature
- coerces each argument to declared parameter type
- rejects unsupported pointer escapes from stack locals

Phase 4 no longer enforces two-argument ABI limits.

## IR Shape

Calls lower to:

- `TypedExprKind::Call`
- `IrInstr::Call { dst, function, args }`

Argument expressions lower in source order. Any subexpression result needed across nested calls is kept in IR temps.

## Backend Lowering

Backend call lowering for `call f(a, b, c)`:

1. load already-lowered argument operands in IR order
2. push each argument byte to software stack
3. emit `CALL`
4. subtract callee argument bytes from `SP`
5. write return value into frame temp when destination exists

## Nested Calls

Nested calls rely on two Phase 4 properties:

- caller cleanup is single-owner and happens only after return
- IR temps live in frame storage, so one invocation cannot overwrite another invocation's temps

This is what allows shapes like:

- `a(b(c(1)))`
- `(lhs + rhs) + f(tmp)`
- `f(a + b) + f(a - b)`

## Return Capture

When a call produces a value:

- low byte comes from `W`
- high byte comes from `return_high` for 16-bit/pointer results
- backend stores that value into frame temp storage if IR requested `dst`

## Non-goals

Phase 4 call lowering still does not support:

- indirect calls
- function pointers
- recursion

Phase 18 note:

- indirect calls are now supported through Phase 17 dispatch-ID lowering
- recursion still rejects before IR generation
- runtime stack overflow checks now exist as backend-inserted `--stack-check` guards rather than dedicated call IR
<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
