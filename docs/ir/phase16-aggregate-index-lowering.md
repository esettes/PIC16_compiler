<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Phase 16 Aggregate Index Lowering

Phase 16 keeps multidimensional arrays inside the ordinary typed aggregate model.

## Indexing

For `T a[D0][D1]`, `a[i][j]` lowers with the row-major formula:

- `((i * D1) + j) * sizeof(T)`

That byte offset is then added to the ordinary base address for:

- globals
- locals
- struct fields
- union fields

## Initializers

- nested initializer lists flatten to row-major scalar slots before IR generation
- missing rows and elements zero-fill
- chained designators overlay those slots before lowering

## Restrictions

- no dedicated multidimensional-array IR opcode is needed
- no pointer-to-array decay model is introduced
- ISR still rejects helper-requiring dynamic index expressions
