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
- recursive aggregate initializer lowering into scalar stores or global byte payloads
- per-function interrupt metadata for backend vector/prologue selection
- optimization-pass-friendly CFG blocks and temp tables

Phase 7 optimization passes:

- constant propagation and folding
- constant branch simplification to direct jumps
- unreachable-block cleanup
- dead code elimination
- temp-slot compaction

Phase 8-13 lowering notes:

- local array/struct initializers are flattened before or during IR lowering into per-slot stores
- global array/struct initializers arrive as byte payloads for backend startup writes
- nested array/struct initializers are recursively flattened into scalar leaves before IR generation
- designated initializers overlay those scalar leaves before IR generation
- whole-struct assignment lowers to byte-wise indirect load/store sequences instead of a dedicated IR opcode

Phase 9 lowering notes:

- `switch` does not add a dedicated IR instruction in this phase
- the IR lowerer evaluates the controlling expression once, then emits a linear equality-compare dispatch chain
- case/default entries become ordinary CFG blocks
- fallthrough is represented with ordinary jumps between successive case blocks
- `break` from switches reuses ordinary jump-to-end CFG lowering
- no jump tables are emitted in this phase because compare chains are simpler to verify on PIC16
- case/default labels nested under unrelated control statements are rejected before IR generation

Phase 10-12 static and pointer lowering notes:

- string-initialized arrays lower to ordinary byte payloads for globals/statics or ordinary per-slot stores for locals
- pointer-initialized string literals lower as synthetic static RAM array symbols plus ordinary address values
- static locals reuse the same startup-initializer path as globals and file-scope statics
- zero-init and initialized static data remain explicit startup-store behavior instead of a separate ROM data section
- pointer-to-pointer values remain ordinary 16-bit pointer temps/operands
- pointer relational comparisons reuse ordinary typed 16-bit compare branches
- pointer subtraction lowers to ordinary 16-bit subtraction plus optional shift-right scaling for 2-byte elements
- explicit `__rom` byte arrays do not become RAM startup payloads; they lower to callable RETLW tables in program memory
- `__rom_read8(table, index)` lowers to one dedicated IR ROM-read instruction instead of a general ROM pointer model

Current Phase 13 limits:

- no multidimensional arrays
- no chained designators such as `.outer.inner = 1`
- no incomplete-struct pointers
- no whole-struct copy inside interrupt handlers
- no program-memory / code-space pointer model
- no direct ROM pointer arithmetic or ROM array indexing syntax
- no pointer subtraction for element sizes larger than 2 bytes

Current detail:

- [phase4-call-lowering.md](/home/settes/cursus/PIC16_compiler/docs/ir/phase4-call-lowering.md:1)
- [phase5-arithmetic-lowering.md](/home/settes/cursus/PIC16_compiler/docs/ir/phase5-arithmetic-lowering.md:1)
- [phase6-isr-lowering.md](/home/settes/cursus/PIC16_compiler/docs/ir/phase6-isr-lowering.md:1)
- [phase8-aggregate-lowering.md](/home/settes/cursus/PIC16_compiler/docs/ir/phase8-aggregate-lowering.md:1)
- [phase9-switch-lowering.md](/home/settes/cursus/PIC16_compiler/docs/ir/phase9-switch-lowering.md:1)
- [phase10-static-initializers.md](/home/settes/cursus/PIC16_compiler/docs/ir/phase10-static-initializers.md:1)
- [phase11-aggregate-initializers.md](/home/settes/cursus/PIC16_compiler/docs/ir/phase11-aggregate-initializers.md:1)
- [phase12-pointer-lowering.md](/home/settes/cursus/PIC16_compiler/docs/ir/phase12-pointer-lowering.md:1)
- [phase13-rom-lowering.md](/home/settes/cursus/PIC16_compiler/docs/ir/phase13-rom-lowering.md:1)

Historical detail:

- [phase2-lowering.md](/home/settes/cursus/PIC16_compiler/docs/ir/phase2-lowering.md:1)
- [phase3-memory.md](/home/settes/cursus/PIC16_compiler/docs/ir/phase3-memory.md:1)
