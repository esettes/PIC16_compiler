<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Phase 9: Backend Switch Codegen

The backend does not receive a special switch opcode in phase 9.

Instead, switch support reuses existing branch lowering:

- IR compare branches for `==`
- ordinary block labels
- ordinary `goto` emission for case entry, fallthrough, and switch-end exits

What backend sees:

- one controlling operand reused across multiple equality checks
- a sequence of compare blocks
- ordinary case/default body blocks
- ordinary jumps for `break`, with nested loop/switch constructs already lowered to the correct innermost exit target

Why this matches PIC16 well:

- no new backend-only control-flow path is needed
- emitted asm stays readable in `.asm` and `.lst`
- existing signed and unsigned compare lowering already covers `char`, `unsigned char`, `int`, `unsigned int`, and enum-backed 16-bit `int`

What is intentionally not implemented:

- jump tables
- dense-range table dispatch
- backend reconstruction of switch directly from AST

ISR interaction:

- switch codegen in ISR is allowed only when the already-validated IR stays inline-safe
- any helper-requiring expressions are still rejected earlier by the Phase 6 interrupt checks
<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
