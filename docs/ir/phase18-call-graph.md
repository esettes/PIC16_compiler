<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Phase 18 Call-Graph And Stack Analysis

Phase 18 strengthens stack analysis without adding a new stack-check IR instruction.

## IR Surface

IR stays same as Phase 17 for calls:

- `IrInstr::Call`
- `IrInstr::IndirectCall`

Runtime stack checks are backend-inserted from frame and argument metadata after IR lowering.

## Call-Graph Inputs

Backend stack analysis expands these edges:

- direct source-level calls
- runtime-helper usage implied by helper-requiring IR arithmetic
- indirect calls through supported function-pointer signature groups
- ISR entry as separate root with extra saved-context cost

For indirect calls:

- if signature group exists, all registered concrete targets are included
- if no target group is known, report marks that target set as unknown

## Stack Metrics

Per function, analysis records:

- argument bytes
- local bytes
- temp bytes
- frame bytes
- helper extra bytes
- maximum nested stack bytes
- maximum call depth
- direct callees
- indirect target groups

Global summary records:

- stack base
- stack limit
- stack capacity
- static max stack usage
- max call depth
- ISR frame bytes
- ISR context bytes
- function-pointer group counts

## Recursion Policy

Recursion is still rejected before backend analysis.

Meaning:

- direct recursion diagnosed in semantic analysis
- mutual recursion diagnosed in semantic analysis
- backend report assumes accepted programs have acyclic user call graph

`--stack-check` does not change this policy in Phase 18.

## Why No New IR Check Opcode

Phase 18 keeps stack checks out of IR because:

- check sequence depends on final ABI frame/arg layout
- helper and dispatcher paths are backend-specific
- PIC16 compare/branch sequence belongs naturally in backend lowering
