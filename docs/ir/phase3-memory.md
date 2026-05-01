<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Phase 3 IR Memory Lowering

## Goal

Phase 3 adds explicit memory-oriented IR so arrays and pointers are not frontend-only concepts.

## Typed Frontend Model

The semantic layer now distinguishes:

- lvalues
- rvalues

Relevant typed expression forms:

- named symbols as lvalues
- array decay
- address-of
- dereference
- assignment to either direct symbols or dereferenced pointers

`sizeof` resolves during semantic analysis to a typed `unsigned int` literal, so it does not survive into IR.

## IR Additions

Phase 3 extends the IR with:

- `IrInstr::AddrOf`
- `IrInstr::LoadIndirect`
- `IrInstr::StoreIndirect`

These nodes sit alongside the existing integer operations from Phase 2.

## Lowering Rules

### Addressable Objects

- `symbol` in value position:
  - scalar objects lower as direct operands
  - arrays lower through explicit decay
- `&symbol` lowers to `AddrOf`
- `&*ptr` reuses the pointer value after typed lowering

### Memory Reads

- `*ptr` in value position lowers to `LoadIndirect`
- `a[i]` lowers to:
  - array decay
  - scaled pointer arithmetic
  - `LoadIndirect`

### Memory Writes

- `symbol = value` still lowers to direct `Store`
- `*ptr = value` lowers to `StoreIndirect`
- `a[i] = value` lowers to:
  - array decay
  - scaled pointer arithmetic
  - `StoreIndirect`

## Why Explicit Memory Ops

This design keeps the IR close to the target realities without leaking PIC16 opcodes into the frontend:

- the frontend reasons about C lvalues/rvalues
- the IR reasons about address materialization and indirect memory
- the backend handles `FSR/INDF`, `STATUS.IRP`, and bank/page details

The result is still reusable for future PIC16 mid-range descriptors without duplicating frontend or device logic.
<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
