<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Phase 11 Aggregate Initializers

Phase 11 keeps aggregate support inside the existing typed-IR pipeline.

## Strategy

- semantic analysis computes packed layout for nested structs and array fields
- aggregate initializers are recursively flattened into scalar leaf assignments
- global/static aggregate initializers become byte payloads
- local aggregate initializers become ordinary per-slot stores
- designated initializers overlay the same scalar leaf map before IR lowering

## Struct Copy

Whole-struct assignment does not add a dedicated aggregate-copy IR instruction.

Instead the lowerer emits:

1. destination address materialization
2. source address materialization
3. byte-wise indirect load/store pairs for the full struct size

This keeps the IR/backed layering unchanged and reuses existing indirect-memory code paths.

## Zero-Fill

- zero-fill is represented before IR generation by seeding every scalar leaf with zero
- explicit initializer entries then overwrite those leaves
- global/static byte payload generation therefore stays little-endian and deterministic

## Limits

- no multidimensional arrays
- no chained designators
- no whole-struct copy inside interrupt handlers
<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
