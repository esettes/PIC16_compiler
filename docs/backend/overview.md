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

Current backend phase: **Phase 18 stack safety, call-graph analysis, and stack-usage reporting on top of Phase 17 controlled function pointers and indirect dispatch, Phase 16 multidimensional arrays and aggregate polish, Phase 15 named union support and basic unsigned bitfields, Phase 14 richer program-memory usability, Phase 13 explicit program-memory const/string/table lowering, Phase 12 richer data-space pointers, Phase 11 aggregate completeness, Phase 10 static-data cleanup, Phase 9 control-flow coverage, Phase 8 aggregate/type-aware lowering, Phase 7 optimization, Phase 6 interrupts, Phase 5 arithmetic helpers, and the Phase 4 Stack-first ABI**

Backend owns:

- software stack helper slots: `stack_ptr`, `frame_ptr`
- stack bound symbols: `__stack_base`, `__stack_limit`, `__stack_ptr`, `__frame_ptr`
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
- byte-wise whole-struct and whole-union copy lowering through existing indirect memory instructions
- union field accesses through shared aggregate base-address lowering
- bitfield read-modify-write lowering through ordinary mask/shift/indirect-store machinery
- row-major multidimensional aggregate offset lowering for locals, globals, and nested aggregate fields
- switch compare-chain blocks through the ordinary branch emitter
- pointer compare/subtract reuse ordinary 16-bit compare/arithmetic lowering
- per-signature generated function-pointer dispatchers with dispatch-ID call lowering
- optional inline stack growth checks before frame growth and argument pushes
- generated `__stack_overflow_trap` infinite-loop handler when stack checks are enabled
- per-function stack report rendering with helper, ISR, and function-pointer target-set accounting
- no backend jump tables and no backend-side recovery of labels nested under unrelated control statements in phase 9
- no backend-side recovery for chained designators or incomplete-struct/union pointers; those stay frontend diagnostics
- bank/page reuse tracking

Current call contract:

- caller pushes args left-to-right
- callee saves caller `FP`
- callee sets `FP` to callee arg base
- callee allocates locals + temps above saved `FP`
- callee restores `SP` to caller arg top
- callee restores caller `FP`
- caller subtracts argument bytes after return
- when `--stack-check` is enabled, caller-side argument growth is guarded before bytes are pushed

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
- helper-call argument growth is guarded too when `--stack-check` is enabled

Current backend docs:

- [phase4-stack-first-abi.md](phase4-stack-first-abi.md)
- [phase4-stack-model.md](phase4-stack-model.md)
- [phase5-helper-calling.md](phase5-helper-calling.md)
- [phase6-interrupts.md](phase6-interrupts.md)
- [phase12-string-pointer-data.md](phase12-string-pointer-data.md)
- [../runtime/phase5-arithmetic-helpers.md](../runtime/phase5-arithmetic-helpers.md)
- [../ir/phase5-arithmetic-lowering.md](../ir/phase5-arithmetic-lowering.md)
- [../ir/phase4-call-lowering.md](../ir/phase4-call-lowering.md)

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

- [optimization.md](optimization.md)

Phase 8 backend docs:

- [phase8-struct-layout.md](phase8-struct-layout.md)

Phase 9 backend docs:

- [phase9-switch-codegen.md](phase9-switch-codegen.md)

Phase 10 backend docs:

- [phase10-data-layout.md](phase10-data-layout.md)

Phase 11-18 backend docs:

- [phase11-aggregate-copy.md](phase11-aggregate-copy.md)
- [phase13-rom-data-layout.md](phase13-rom-data-layout.md)
- [phase14-retlw-tables.md](phase14-retlw-tables.md)
- [phase15-bitfield-codegen.md](phase15-bitfield-codegen.md)
- [phase16-aggregate-layout.md](phase16-aggregate-layout.md)
- [phase17-dispatcher.md](phase17-dispatcher.md)
- [phase18-stack-safety.md](phase18-stack-safety.md)

Historical docs:

- [phase2-abi.md](phase2-abi.md)
- [phase3-memory-model.md](phase3-memory-model.md)
