<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

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
- indirect-call instructions with normalized function-pointer signatures
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

Phase 8-17 lowering notes:

- local array/struct initializers are flattened before or during IR lowering into per-slot stores
- global array/struct initializers arrive as byte payloads for backend startup writes
- nested array/struct initializers are recursively flattened into scalar leaves before IR generation
- designated initializers overlay those scalar leaves before IR generation
- whole-struct and whole-union assignment lower to byte-wise indirect load/store sequences instead of dedicated IR opcodes

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
- direct ROM indexing and `__rom_read16(table, index)` reuse the same typed ROM-read path

Phase 15 aggregate lowering notes:

- union field access lowers through ordinary base address plus constant zero offset
- union initializers zero-fill the whole storage byte range, then overlay the selected field bytes
- bitfield reads lower to ordinary load + shift-right + mask operations
- bitfield writes lower to ordinary read-modify-write sequences on the containing byte/word storage unit
- no dedicated union-copy or bitfield IR instruction is introduced; existing indirect copy/arithmetic ops stay sufficient

Phase 16 aggregate indexing notes:

- repeated multidimensional indexing lowers through row-major byte-offset composition
- chained designators resolve to one final scalar or aggregate target before IR generation
- multidimensional local/global initializers flatten to row-major scalar slots before backend startup or local-store lowering

Phase 17 indirect-call notes:

- supported function-pointer calls lower to one explicit `IndirectCall` instruction
- direct function names and `&function` lower to stable per-signature dispatch IDs
- indirect-call recursion and stack-depth analysis expand across every known target in the matching signature group
- no raw code pointer values or computed PIC16 calls are introduced

Phase 18 stack-analysis notes:

- Phase 18 does not add a dedicated stack-check IR instruction
- runtime stack guards are backend-inserted from frame/arg metadata, not frontend-only annotations
- call-graph analysis expands across direct calls, helper-triggering operations, ISR roots, and known function-pointer target groups
- recursion diagnostics remain semantic; backend stack reports assume acyclic call graphs after semantic validation

Current Phase 18 limits:

- no incomplete-struct/union pointers
- no whole-aggregate copy inside interrupt handlers
- no program-memory / code-space pointer model
- no direct ROM pointer arithmetic
- no pointer subtraction for element sizes larger than 2 bytes
- no anonymous nested aggregate fields
- no signed bitfields
- no pointer-to-function-pointer object model
- no function-pointer arithmetic or relational comparisons
- no indirect calls inside interrupt handlers
- no recursion, even with `--stack-check`

Current detail:

- [phase4-call-lowering.md](phase4-call-lowering.md)
- [phase5-arithmetic-lowering.md](phase5-arithmetic-lowering.md)
- [phase6-isr-lowering.md](phase6-isr-lowering.md)
- [phase8-aggregate-lowering.md](phase8-aggregate-lowering.md)
- [phase9-switch-lowering.md](phase9-switch-lowering.md)
- [phase10-static-initializers.md](phase10-static-initializers.md)
- [phase11-aggregate-initializers.md](phase11-aggregate-initializers.md)
- [phase12-pointer-lowering.md](phase12-pointer-lowering.md)
- [phase13-rom-lowering.md](phase13-rom-lowering.md)
- [phase14-rom-read-lowering.md](phase14-rom-read-lowering.md)
- [phase15-aggregate-lowering.md](phase15-aggregate-lowering.md)
- [phase16-aggregate-index-lowering.md](phase16-aggregate-index-lowering.md)
- [phase17-indirect-call-lowering.md](phase17-indirect-call-lowering.md)
- [phase18-call-graph.md](phase18-call-graph.md)

Historical detail:

- [phase2-lowering.md](phase2-lowering.md)
- [phase3-memory.md](phase3-memory.md)
