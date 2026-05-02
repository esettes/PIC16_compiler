<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Phase 17 Indirect Call Lowering

Phase 17 keeps controlled indirect calls explicit in IR.

## Model

- direct named calls still lower to ordinary `Call`
- function-pointer calls lower to `IndirectCall`
- `IndirectCall` carries:
  - callee operand
  - normalized function-pointer signature
  - ordinary argument operands
  - optional destination temp

## Dispatch IDs

- direct function names and `&function` lower to typed 16-bit dispatch-ID values
- one signature group records the concrete target set for that function-pointer type
- stack-depth analysis expands an indirect call across every target in the matching signature group

## Restrictions

- no raw code pointer values appear in IR
- no pointer arithmetic over function addresses is introduced
- ISR still rejects indirect-call lowering before backend codegen
