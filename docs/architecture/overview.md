<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# General Architecture

Phase 22 extends the existing architecture with explicit fixed-point scalar support while preserving the Stack-first ABI and PIC16 `midrange14` backend. The compiler treats Q8.8 values as 2-byte raw fixed storage and Q16.16 values as 4-byte raw fixed storage across frontend typing, IR temps, stack frames, globals, aggregates, and simulator validation.

Phase 21 32-bit integers remain the raw intermediate foundation for Q8.8 multiply/divide. Q16.16 multiply/divide are deferred because a correct implementation needs a wider intermediate than Phase 21 currently provides.

`pic16cc` separates:

- C frontend
- internal IR
- shared PIC16 backend
- device descriptor layer
- final `.hex` emission

Target backend is classic 14-bit PIC16 mid-range family, not a generic 8-bit CPU model.

Current Phase 22 keeps that split intact while extending:

- stack-first caller-pushed ABI
- fixed-point frontend typing, raw constructors, casts, and diagnostics
- fixed-point IR lowering through raw integer storage and existing helper paths
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
- Phase 16 row-major multidimensional aggregate layout, indexing, and chained designator handling
- Phase 17 controlled function-pointer typing plus dispatch-ID indirect-call lowering
- Phase 18 target-aware stack bounds, optional runtime stack guards, and stronger call-graph/stack-report visibility
- Phase 19 test-only execution validation through an internal PIC16 core emulator fed from generated Intel HEX output
- Phase 20 optional `pic16-sim` CLI for running HEX files, resolving map symbols, printing state, and tracing instructions
- Phase 21 controlled 32-bit integer storage, ABI, helpers, and simulator validation
- Phase 22 explicit fixed-point Q8.8/Q16.16 typing and simulator-validated fixed arithmetic
- PIC16 banking/paging without backend duplication per device

Phase 22 keeps the compiler layering unchanged:

- frontend still produces typed trees only
- IR still stays target-aware but encoding-agnostic
- backend still owns all real PIC16 code generation
- emulator reads backend-produced HEX artifacts after compilation; it does not replace codegen
- `pic16-sim` is optional tooling and does not alter normal `picc` behavior

Execution-validation layer:

- `src/sim/mod.rs`: minimal PIC16 core emulator for the emitted instruction subset
- `src/sim_cli.rs`: user-facing simulator CLI parsing, map loading, trace output, and diagnostics
- `tests/execution_sim.rs`: compile -> HEX -> simulate -> assert runtime state regressions
- `tests/sim_cli.rs`: command-level `pic16-sim` coverage
- simulator remains optional; normal `picc` CLI flow is unchanged

See:

- [DESIGN.md](../../DESIGN.md)
- [Backend Overview](../backend/overview.md)
- [IR Overview](../ir/overview.md)
- [Testing: Phase 19 Emulator](../testing/phase19-emulator.md)
- [PIC16 Core Emulator](../sim/pic16-core-emulator.md)
- [PIC16 Simulator CLI](../sim/pic16-sim-cli.md)
<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
