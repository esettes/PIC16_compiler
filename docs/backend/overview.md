<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# PIC16 `midrange14` Backend

Shared backend responsibilities:

- instruction selection
- direct-bank selection
- indirect-bank selection
- paging
- startup
- IR -> PIC16 asm lowering
- 14-bit word encoding

Current backend phase: **Phase 13 explicit program-memory const/string/table lowering on top of Phase 12 richer data-space pointers, Phase 11 aggregate completeness, Phase 10 static-data cleanup, Phase 9 control-flow coverage, Phase 8 aggregate/type-aware lowering, Phase 7 optimization, Phase 6 interrupts, Phase 5 arithmetic helpers, and the Phase 4 Stack-first ABI**

Backend owns:

- software stack helper slots: `stack_ptr`, `frame_ptr`
- return helper slot: `return_high`
- short-lived scratch slots: `scratch0`, `scratch1`
- caller-pushed stack argument lowering
- per-call frame lowering for locals and IR temps
- `FSR/INDF` indirect access for pointers and frame storage
- Phase 5 runtime helper emission for multiply/divide/modulo and dynamic shifts
- interrupt vector emission and ISR dispatch
- ISR-specific save/restore and `retfie` lowering
- Phase 7 peephole cleanup and helper fast-path selection
- packed struct field offset lowering for `.` / `->`
- startup writes for pre-flattened global/static aggregate initializer bytes
- startup clears for zero-init globals/statics
- startup comments and map labels that annotate const/static data
- startup address writes for pointer-valued globals/statics
- RAM-backed string-literal data symbols and a dedicated map section for them
- RETLW-backed ROM table emission in program memory for explicit `const __rom` byte arrays
- inline ROM-read lowering through generated ROM table calls
- separate ROM symbol map section
- byte-wise whole-struct copy lowering through existing indirect memory instructions
- switch compare-chain blocks through the ordinary branch emitter
- pointer compare/subtract reuse ordinary 16-bit compare/arithmetic lowering
- no backend jump tables and no backend-side recovery of labels nested under unrelated control statements in phase 9
- no backend-side recovery for chained designators or incomplete-struct pointers; those stay frontend diagnostics
- bank/page reuse tracking

Current call contract:

- caller pushes args left-to-right
- callee saves caller `FP`
- callee sets `FP` to callee arg base
- callee allocates locals + temps above saved `FP`
- callee restores `SP` to caller arg top
- callee restores caller `FP`
- caller subtracts argument bytes after return

Current return contract:

- 8-bit: `W`
- 16-bit: `W` low + `return_high` high
- pointer: same as 16-bit integer

Phase 5 helper contract:

- helpers use same stack-first ABI as normal functions
- caller pushes helper args left-to-right, low byte first inside each scalar
- caller cleans helper arg bytes after return
- helpers may mutate their own arg slots as working storage
- helper locals/count/flags live above saved `FP` in helper frame storage
- helper labels are emitted only when used and appear in `.map` / `.lst`

Current backend docs:

- [phase4-stack-first-abi.md](/home/settes/cursus/PIC16_compiler/docs/backend/phase4-stack-first-abi.md:1)
- [phase4-stack-model.md](/home/settes/cursus/PIC16_compiler/docs/backend/phase4-stack-model.md:1)
- [phase5-helper-calling.md](/home/settes/cursus/PIC16_compiler/docs/backend/phase5-helper-calling.md:1)
- [phase6-interrupts.md](/home/settes/cursus/PIC16_compiler/docs/backend/phase6-interrupts.md:1)
- [phase12-string-pointer-data.md](/home/settes/cursus/PIC16_compiler/docs/backend/phase12-string-pointer-data.md:1)
- [../runtime/phase5-arithmetic-helpers.md](/home/settes/cursus/PIC16_compiler/docs/runtime/phase5-arithmetic-helpers.md:1)
- [../ir/phase5-arithmetic-lowering.md](/home/settes/cursus/PIC16_compiler/docs/ir/phase5-arithmetic-lowering.md:1)
- [../ir/phase4-call-lowering.md](/home/settes/cursus/PIC16_compiler/docs/ir/phase4-call-lowering.md:1)

Phase 6 interrupt contract:

- ISR syntax is `void __interrupt isr(void)`
- one ISR per program
- ISR uses the same software-stack frame machinery after saving context
- vector at `0x0004` dispatches to a page-safe ISR stub
- default no-ISR vector is `retfie`
- ISR saves `W`, `STATUS`, `PCLATH`, `FSR`, `return_high`, `scratch0`, `scratch1`, `stack_ptr`, `frame_ptr`
- ISR ends with `retfie`

Phase 6 restrictions:

- no normal function calls inside ISR
- no Phase 5 helper calls inside ISR
- helper-requiring `*`, `/`, `%`, and dynamic shifts are rejected during semantic analysis

Phase 7 optimization responsibilities:

- remove redundant PIC16 instruction pairs and duplicate writes
- avoid helper calls for unsigned power-of-two divide/modulo when inline cheaper
- compact visible bank-bit changes to only the bits that actually changed
- preserve page-selection correctness while dropping duplicate `setpage`
- improve `.map` readability by grouping user code, helpers, vectors, ABI/stack data, and ISR context

Phase 7 backend docs:

- [optimization.md](/home/settes/cursus/PIC16_compiler/docs/backend/optimization.md:1)

Phase 8 backend docs:

- [phase8-struct-layout.md](/home/settes/cursus/PIC16_compiler/docs/backend/phase8-struct-layout.md:1)

Phase 9 backend docs:

- [phase9-switch-codegen.md](/home/settes/cursus/PIC16_compiler/docs/backend/phase9-switch-codegen.md:1)

Phase 10 backend docs:

- [phase10-data-layout.md](/home/settes/cursus/PIC16_compiler/docs/backend/phase10-data-layout.md:1)

Phase 11 backend docs:

- [phase11-aggregate-copy.md](/home/settes/cursus/PIC16_compiler/docs/backend/phase11-aggregate-copy.md:1)
- [phase13-rom-data-layout.md](/home/settes/cursus/PIC16_compiler/docs/backend/phase13-rom-data-layout.md:1)

Historical docs:

- [phase2-abi.md](/home/settes/cursus/PIC16_compiler/docs/backend/phase2-abi.md:1)
- [phase3-memory-model.md](/home/settes/cursus/PIC16_compiler/docs/backend/phase3-memory-model.md:1)
<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
