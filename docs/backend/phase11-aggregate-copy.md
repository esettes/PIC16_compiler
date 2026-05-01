<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Phase 11 Aggregate Copy

Phase 11 does not add a backend-only aggregate path.

## Backend Role

- honor packed nested struct and array-field offsets already computed by the frontend
- emit startup writes for flattened nested aggregate byte payloads
- emit byte-wise indirect copies for whole-struct assignment

## Struct Copy Lowering Shape

Whole-struct assignment reaches the backend as existing indirect-memory IR operations:

- address materialization for source and destination
- repeated byte-pointer offset additions
- indirect byte loads
- indirect byte stores

No jump tables, helper calls, or backend AST inspection are introduced for this phase.

## Why Byte-Wise Copy

- matches packed struct layout exactly
- works for nested structs and array fields without new runtime helpers
- fits PIC16 `FSR/INDF` addressing naturally
- stays easy to inspect in `.asm`, `.map`, and `.lst` artifacts

## Limits

- whole-struct copy is still rejected inside interrupt handlers
- no backend support for incomplete-struct pointers
- no backend support for multidimensional arrays or chained designators because those are rejected earlier
<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
