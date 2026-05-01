<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# General Architecture

`pic16cc` separates:

- C frontend
- internal IR
- shared PIC16 backend
- device descriptor layer
- final `.hex` emission

Target backend is classic 14-bit PIC16 mid-range family, not a generic 8-bit CPU model.

Current Phase 15 keeps that split intact while extending:

- stack-first caller-pushed ABI
- per-call frame storage for locals and IR temps
- typed IR call lowering for arbitrary argument counts
- explicit pointer and frame access through `FSR/INDF`
- arithmetic runtime helpers without replacing shared `midrange14` backend
- Phase 7 IR/backend optimization passes
- Phase 8 `typedef`/`enum`/packed-`struct` frontend support
- Phase 8 flat aggregate initializer analysis and lowering
- Phase 8 explicit cast validation for supported scalar/data-pointer forms
- Phase 9 `switch` / `case` / `default` parsing and semantic validation
- Phase 9 compare-chain lowering into ordinary CFG branches instead of backend AST shortcuts
- Phase 9 rejection of `case` / `default` labels nested under unrelated control statements
- Phase 10 startup-time RAM initialization for globals, statics, const objects, and static locals
- Phase 10 map/listing annotations for initialized or zeroed static data
- Phase 11 arrays inside structs and nested struct-field layout
- Phase 11 nested aggregate and designated initializer analysis before IR generation
- Phase 11 byte-wise whole-struct copy lowering through existing indirect memory machinery
- Phase 12 nested data-space pointer typing and const-qualified pointer forms
- Phase 12 pointer relational compare/subtract lowering through ordinary typed 16-bit operations
- Phase 12 RAM-backed string-literal objects for pointer initialization
- Phase 13 explicit ROM address-space tagging for objects without changing data-space pointers
- Phase 13 RETLW-backed program-memory table emission plus `__rom_read8()` lowering
- Phase 14 direct ROM indexing, 16-bit ROM tables, and `__rom_read16()` lowering
- Phase 15 named union layout/copy plus basic unsigned bitfield lowering without backend layering breaks
- PIC16 banking/paging without backend duplication per device

See:

- [DESIGN.md](../../DESIGN.md)
- [Backend Overview](../backend/overview.md)
- [IR Overview](../ir/overview.md)
<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
