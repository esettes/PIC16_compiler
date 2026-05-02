<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Phase 18 Stack Safety

Phase 18 adds visibility and optional runtime checking to existing Stack-first ABI.

## Stack Bounds

Software stack still grows upward in data RAM.

Backend now exposes:

- `__stack_base`
- `__stack_limit`
- `__stack_ptr`
- `__frame_ptr`

Rules:

- `__stack_base` is first usable stack byte
- `__stack_limit` is exclusive high bound
- `__stack_ptr` and `__frame_ptr` name current ABI helper state
- legacy `__stack.base` / `__stack.end` aliases remain in map output for compatibility

Target descriptors define usable RAM ranges. Backend computes stack region from that descriptor data before frame lowering.

## Runtime Checks

`--stack-check` enables inline growth guards.

Backend inserts checks before:

- normal function frame allocation
- runtime-helper frame allocation
- normal-call argument pushes
- runtime-helper argument pushes
- function-pointer dispatcher-call argument pushes

Check model:

1. compute candidate `SP + growth`
2. compare candidate against exclusive `__stack_limit`
3. branch to `__stack_overflow_trap` on overflow

No helper call is used for stack checking.

## Trap

`__stack_overflow_trap` is emitted only when `--stack-check` is enabled.

Behavior:

- one label
- one self-looping `goto`
- no helper call
- no attempt to recover

This keeps trap path predictable and ISR-safe.

## Reporting

Phase 18 also renders stack reports from backend analysis.

Report includes:

- target name
- stack base / limit / capacity
- static max stack usage
- runtime-check status
- ISR frame bytes
- ISR saved-context bytes
- per-function frame, locals, temps, helper extra, max stack, and max call depth
- function-pointer target-set expansion

## ISR Notes

- ISR context save size is counted separately from ISR frame bytes
- function-pointer calls remain rejected inside ISR
- helper-requiring ISR expressions remain rejected
- dynamic stack checks are still inline-safe because they do not call helpers

## Limits

- recursion still rejected
- reports are conservative when a function-pointer signature group has no known target set
- stack checks guard bounded acyclic growth only
