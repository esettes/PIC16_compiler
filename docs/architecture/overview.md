# General Architecture

`pic16cc` separates:

- C frontend
- internal IR
- shared PIC16 backend
- device descriptor layer
- final `.hex` emission

Target backend is classic 14-bit PIC16 mid-range family, not a generic 8-bit CPU model.

Current Phase 10 keeps that split intact while extending:

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
- Phase 10 string literal parsing for array initialization only
- Phase 10 startup-time RAM initialization for globals, statics, const objects, and static locals
- Phase 10 map/listing annotations for initialized or zeroed static data
- PIC16 banking/paging without backend duplication per device

See:

- [../../DESIGN.md](/home/settes/cursus/PIC16_compiler/DESIGN.md:1)
- [../backend/overview.md](/home/settes/cursus/PIC16_compiler/docs/backend/overview.md:1)
- [../ir/overview.md](/home/settes/cursus/PIC16_compiler/docs/ir/overview.md:1)
