<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Phase 6 ISR Lowering

Phase status:

- ISR lowering is frozen at Phase 6 for stabilization
- no Phase 7 ISR lowering scope exists in this branch

Phase 6 keeps ISR bodies in the same CFG-based IR as normal functions.

IR additions:

- each `IrFunction` carries `is_interrupt`

Why metadata instead of special ISR instructions:

- ISR body statements still lower through normal typed expressions, branches, stores, loads, and frame temps
- interrupt-ness mainly changes backend entry/exit behavior, not expression semantics

Backend behavior triggered by `is_interrupt`:

- emit dispatch from the interrupt vector
- use ISR save/restore instead of normal prologue/epilogue alone
- return with `retfie`
- exclude ISR from the normal call-graph depth walk, then add one ISR frame on top of worst-case normal depth

Phase 6 semantic restrictions happen before backend lowering:

- no normal calls in ISR
- no runtime-helper-requiring arithmetic in ISR

This keeps IR simple while still carrying the metadata the backend needs.
<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
