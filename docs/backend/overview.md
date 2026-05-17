<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# PIC16 `midrange14` Backend

Phase 32 adds page-aware layout reporting and safe linker relaxation on top of the Phase 31 `PCLATH` page-safety validator. Phase 30 ROM float data, Phase 29 finite-float runtime completion, Phase 26 config/HEX validation, Phase 25 target resource fitting, and Phase 24 dynamic Q16.16/UQ16.16 helpers remain supported.

Phase 21 extends the PIC16 backend to four-byte integer values. The return convention is `W` plus `__abi.return_high`, `__abi.return_upper0`, and `__abi.return_upper1`. Multi-byte carry/borrow codegen is shared by 8-, 16-, 32-bit, and fixed raw paths.

See `docs/runtime/phase24-q16-16-dynamic-helpers.md`, `docs/ir/phase24-fixed-dynamic-lowering.md`, `docs/backend/phase23-fixed-rom-tables.md`, `docs/backend/phase22-fixed-codegen.md`, and `docs/backend/phase21-long-codegen.md`.

Shared backend responsibilities:

- instruction selection
- direct-bank selection
- indirect-bank selection
- paging
- startup
- IR -> PIC16 asm lowering
- 14-bit word encoding

Current backend phase: **Phase 32 page-aware layout and linker relaxation on top of Phase 31 page-safety, Phase 30 ROM float table codegen, Phase 29 float compare/conversion helpers, Phase 26 config/HEX workflow, Phase 25 resource fitting, Phase 24 dynamic Q16.16 helper codegen, Phase 23 fixed ROM table codegen, Phase 22 fixed-point codegen, Phase 21 32-bit integer codegen, Phase 20 simulator tooling, Phase 19 execution validation, Phase 18 stack safety, Phase 17 controlled function pointers, and the earlier backend phases**

Backend owns:

- software stack helper slots: `stack_ptr`, `frame_ptr`
- stack bound symbols: `__stack_base`, `__stack_limit`, `__stack_ptr`, `__frame_ptr`
- return helper slots: `return_high`, `return_upper0`, `return_upper1`
- short-lived scratch slots: `scratch0`, `scratch1`
- caller-pushed stack argument lowering
- per-call frame lowering for locals and IR temps
- `FSR/INDF` indirect access for pointers and frame storage
- Phase 5 runtime helper emission for multiply/divide/modulo and dynamic shifts
- fixed-point raw storage, casts, comparisons, and Q8.8 helper-backed arithmetic
- page-safe Q16.16/UQ16.16 dynamic multiply/divide helpers
- page-safe conditional branch emission for PIC16 skip instructions
- final layout validation of `goto` / `call` edges against tracked `PCLATH`
- linker relaxation that removes redundant same-page `setpage` pseudo-ops
- inferred code-section metadata and per-page layout summaries
- target memory fit validation after final encoding
- compact and detailed resource report rendering
- helper/dispatcher/ROM-table contribution reporting
- config-word resolution from `#pragma config` and raw `__config(...)`
- final Intel HEX validation for target range, vectors, config, word width, checksum, and EOF
- fixed-point ROM table reads through `RomRead16`/`RomRead32`
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
- stable code shape consumed by the Phase 19 internal emulator for runtime regression tests
- no backend jump tables and no backend-side recovery of labels nested under unrelated control statements in phase 9
- no backend-side recovery for chained designators or incomplete-struct/union pointers; those stay frontend diagnostics
- bank/page reuse tracking
- `.map` code symbol page metadata and `.lst` page-safe pseudo-op comments

Phase 19 execution-validation relationship:

- backend still emits assembly, machine words, HEX, map, and listing exactly as before
- internal simulator consumes HEX after encoding; it does not bypass backend lowering
- simulator failures on unsupported instructions are treated as backend/test coverage gaps, not as alternate compilation paths

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
- 16-bit and Q8.8: `W` low + `return_high` high
- 32-bit and Q16.16: `W` + `return_high` + `return_upper0` + `return_upper1`
- pointer: same as 16-bit integer

Phase 5 helper contract:

- helpers use same stack-first ABI as normal functions
- caller pushes helper args left-to-right, low byte first inside each scalar
- caller cleans helper arg bytes after return
- helpers may mutate their own arg slots as working storage
- helper locals/count/flags live above saved `FP` in helper frame storage
- helper labels are emitted only when used and appear in `.map` / `.lst`
- helper-call argument growth is guarded too when `--stack-check` is enabled
- Q16.16 helper calls use 8 argument bytes and return 4 raw fixed bytes through the Phase 21 return slots

Current backend docs:

- [phase4-stack-first-abi.md](phase4-stack-first-abi.md)
- [phase4-stack-model.md](phase4-stack-model.md)
- [phase5-helper-calling.md](phase5-helper-calling.md)
- [phase6-interrupts.md](phase6-interrupts.md)
- [phase12-string-pointer-data.md](phase12-string-pointer-data.md)
- [../runtime/phase5-arithmetic-helpers.md](../runtime/phase5-arithmetic-helpers.md)
- [phase21-long-codegen.md](phase21-long-codegen.md)
- [phase22-fixed-codegen.md](phase22-fixed-codegen.md)
- [phase23-fixed-rom-tables.md](phase23-fixed-rom-tables.md)
- [phase25-resource-fitting.md](phase25-resource-fitting.md)
- [phase26-config-word.md](phase26-config-word.md)
- [phase26-hex-validation.md](phase26-hex-validation.md)
- [phase31-page-safety.md](phase31-page-safety.md)
- [pic16-code-layout.md](pic16-code-layout.md)
- [phase32-page-aware-layout.md](phase32-page-aware-layout.md)
- [phase32-linker-relaxation.md](phase32-linker-relaxation.md)
- [../runtime/phase22-fixed-helpers.md](../runtime/phase22-fixed-helpers.md)
- [../runtime/phase24-q16-16-dynamic-helpers.md](../runtime/phase24-q16-16-dynamic-helpers.md)
- [../ir/phase24-fixed-dynamic-lowering.md](../ir/phase24-fixed-dynamic-lowering.md)
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

Phase 27 float backend responsibilities:

- store `float` as 4-byte little-endian f32 bits
- reuse the existing 32-bit return slots
- emit finite float helpers as `float helper` resource contributions
- emit Phase 28 cast helpers `__rt_q16_16_to_f32` and `__rt_f32_to_q16_16`
- emit Phase 29 compare and 32-bit conversion helpers as `float helper` resource contributions
- emit Phase 30 `const __rom float[]` tables as RETLW-backed ROM table contributions
- lower ROM float reads through the existing 32-bit little-endian ROM byte-read path
- inline common `* 2.0f` and `/ 2.0f` exponent adjustments
- let resource fitting reject helper-heavy float programs that do not fit a target

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
- [phase27-float-codegen.md](phase27-float-codegen.md)
- [phase28-float-resource-cost.md](phase28-float-resource-cost.md)
- [phase30-rom-float-tables.md](phase30-rom-float-tables.md)
- [../runtime/phase29-float-compare.md](../runtime/phase29-float-compare.md)
- [../runtime/phase29-float-i32-conversions.md](../runtime/phase29-float-i32-conversions.md)

Phase 19 execution-validation docs:

- [../testing/phase19-emulator.md](../testing/phase19-emulator.md)
- [../sim/pic16-core-emulator.md](../sim/pic16-core-emulator.md)

Historical docs:

- [phase2-abi.md](phase2-abi.md)
- [phase3-memory-model.md](phase3-memory-model.md)
