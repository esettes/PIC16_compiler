# IR

Custom CFG-based IR.

Motivation:

- separate AST from backend
- enable validation and optimization passes
- keep call and memory lowering explicit

Current IR carries:

- typed casts
- typed compare conditions
- explicit address materialization
- explicit indirect load/store
- direct-call instructions with arbitrary argument lists
- typed arithmetic and shift instructions for helper-aware lowering
- member-access-friendly base + constant-offset address computations
- flat aggregate initializer lowering into scalar stores or global byte payloads
- per-function interrupt metadata for backend vector/prologue selection
- optimization-pass-friendly CFG blocks and temp tables

Phase 7 optimization passes:

- constant propagation and folding
- constant branch simplification to direct jumps
- unreachable-block cleanup
- dead code elimination
- temp-slot compaction

Phase 8 lowering notes:

- local array/struct initializers are flattened before or during IR lowering into per-slot stores
- global array/struct initializers arrive as byte payloads for backend startup writes
- whole-struct assignment, designated initializers, and nested aggregate forms are rejected before IR generation

Phase 9 lowering notes:

- `switch` does not add a dedicated IR instruction in this phase
- the IR lowerer evaluates the controlling expression once, then emits a linear equality-compare dispatch chain
- case/default entries become ordinary CFG blocks
- fallthrough is represented with ordinary jumps between successive case blocks
- `break` from switches reuses ordinary jump-to-end CFG lowering
- no jump tables are emitted in this phase because compare chains are simpler to verify on PIC16
- case/default labels nested under unrelated control statements are rejected before IR generation

Current detail:

- [phase4-call-lowering.md](/home/settes/cursus/PIC16_compiler/docs/ir/phase4-call-lowering.md:1)
- [phase5-arithmetic-lowering.md](/home/settes/cursus/PIC16_compiler/docs/ir/phase5-arithmetic-lowering.md:1)
- [phase6-isr-lowering.md](/home/settes/cursus/PIC16_compiler/docs/ir/phase6-isr-lowering.md:1)
- [phase8-aggregate-lowering.md](/home/settes/cursus/PIC16_compiler/docs/ir/phase8-aggregate-lowering.md:1)
- [phase9-switch-lowering.md](/home/settes/cursus/PIC16_compiler/docs/ir/phase9-switch-lowering.md:1)

Historical detail:

- [phase2-lowering.md](/home/settes/cursus/PIC16_compiler/docs/ir/phase2-lowering.md:1)
- [phase3-memory.md](/home/settes/cursus/PIC16_compiler/docs/ir/phase3-memory.md:1)
