<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Phase 12 Pointer Lowering

Phase 12 keeps pointer work inside the existing typed IR instead of adding a new pointer-specific IR instruction set.

Lowering model:

- pointer values remain ordinary 16-bit typed operands and temps
- address-of still lowers through `IrInstr::AddrOf`
- dereference still lowers through `IrInstr::LoadIndirect` / `IrInstr::StoreIndirect`
- pointer-to-pointer uses the same indirect load/store path recursively
- pointer relational comparisons lower through ordinary typed compare branches
- pointer subtraction lowers through:
  - 16-bit subtraction on raw RAM addresses
  - optional right-shift-by-1 scaling for 2-byte element types

String literals:

- string literals used in pointer initializers become synthetic static array symbols before IR generation
- local pointer initializers use ordinary address values of those symbols
- global/static pointer initializers are stored as symbolic-address startup data, not as frontend-only placeholders

Non-goals in this phase:

- no dedicated switch/table-like pointer opcode
- no program-memory/code-space pointer lowering
- no helper-based divide scaling for pointer subtraction on element sizes larger than 2 bytes
<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
