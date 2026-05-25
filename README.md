<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# pic16cc

`pic16cc` is an experimental Rust compiler for classic 14-bit PIC16 mid-range MCUs. The pipeline is native end to end:

Installed CLI executable names:

- `picc`: compiler
- `pic16-sim`: optional simulator/debugging tool

1. preprocessing
2. lexing
3. parsing
4. semantic analysis
5. typed IR lowering
6. IR optimization
7. shared PIC16 `midrange14` lowering/codegen
8. 14-bit instruction encoding
9. Intel HEX emission

Supported devices:

- `PIC16F628A`
- `PIC16F877A`

Outputs:

- programmable Intel HEX (`.hex`)
- symbol map (`.map`)
- listing (`.lst`)
- optional token, AST, IR, and assembly dumps

## Current Status

Current implementation is **Phase 43: finite table-driven `sinf` / `cosf`, on top of Phase 42 float compare/minmax compaction, Phase 41 finite `fminf` / `fmaxf`, Phase 40 numeric accuracy harness and expanded precise-profile finite `sqrtf`, Phase 39 precise-profile `sqrtf`, Phase 38 math accuracy profiles, Phase 37 finite `sqrtf`, Phase 36 minimal finite `math.h` for `float`, Phase 35 fixed/float helper compaction, Phase 34 integer helper compaction, Phase 33 runtime helper reporting, Phase 32 page-aware code layout, Phase 31 backend page-safety, Phase 30 ROM float tables, Phase 29 dynamic float comparisons and 32-bit integer conversions, Phase 28 float hardening, Phase 27 basic finite software `float`, Phase 26 device configuration/HEX validation/programmer workflow, Phase 25 target resource limits, and the earlier frontend/backend phases**.

Phase 43 scope:

- `include/math.h` exposes the finite float-only subset `fabsf`, `truncf`, `floorf`, `ceilf`, `roundf`, `sqrtf`, `fminf`, `fmaxf`, `sinf`, and `cosf`
- `sinf` / `cosf` interpret input as radians and use internal ROM quarter-wave tables for validated finite points
- compact trig tolerance is `<= 0.10`; balanced trig tolerance is `<= 0.05` for simulator-validated points in `[-2π, +2π]`
- dynamic precise-profile `sinf` / `cosf` is deferred and diagnoses instead of silently falling back
- internal ROM math tables such as `__rt_math_sin_qwave_table_balanced` are emitted only when dynamic trig helpers are used and appear in reports, `.map`, and `.lst`
- dynamic float comparisons use compact raw finite f32 ordering in `__rt_f32_cmp` instead of Q16.16 conversion
- `fminf` / `fmaxf` lower to `__rt_f32_cmp` plus local compare/select code; no `__rt_f32_min` or `__rt_f32_max` helper body is emitted
- constant math calls fold when their argument is a finite compile-time `float`
- `roundf` uses half-away-from-zero behavior; `sqrtf(x < 0)` returns `0.0f` because NaN/Inf are outside the finite model
- `fminf` / `fmaxf` select the lower/higher finite operand through the shared finite f32 compare helper; signed-zero tie behavior is implementation-defined within the finite-only model
- dynamic `sqrtf` returns exact documented values for validated common inputs and uses a compact approximation fallback for other positive finite values; it is not correctly-rounded IEEE-754 `sqrt`
- `--math-profile compact|balanced|precise` selects finite math accuracy policy independently from `--runtime-profile`
- `compact` uses the Phase 37 compact approximation; `balanced` currently uses the same compact helper and reports that honestly
- `precise` emits a larger `precise_table_refined` helper that returns refined raw f32 results for a broader validated set: `0.25f`, `0.5f`, `1.0f`, `2.0f`, `3.0f`, `4.0f`, `5.0f`, `8.0f`, `9.0f`, `10.0f`, `16.0f`, `25.0f`, `36.0f`, `49.0f`, and `64.0f`
- simulator accuracy tests compare dynamic precise `sqrtf` against host `f32::sqrt` and require the documented positive range to stay within `±0.03125`
- `precise` is at least as accurate as compact for tested non-perfect roots and strictly better for multiple tested roots, but it is still finite-only and not a correctly-rounded IEEE-754 implementation
- math helpers are demand-pruned, categorized as `math helper`, and reported in `--size`, `--memory-report`, stack data, `.map`, and `.lst`
- compare/minmax compaction is validated under balanced and small runtime profiles
- `fabsf` may be used in ISRs only when it lowers inline; other math helpers, including `sqrtf`, `fminf`, `fmaxf`, `sinf`, and `cosf`, remain rejected in ISRs
- no `double`, no full ISO C `math.h`, no `tanf`/`atanf`/`powf`/`expf`/`logf`, no errno/fenv, and no IEEE NaN/Inf promise

Phase 35 scope:

- centralized runtime helper catalog metadata for category, ABI size, stack frame, required-by text, dependency list, and target constraints
- helper dependency graph validation before emitting runtime bodies
- helper pruning remains demand-driven: literal-only `float`, `long`, and fixed-point declarations do not pull arithmetic helpers
- `--runtime-profile small` now selects compact unsigned/signed 32-bit div/mod wrappers around shared `__rt_u32_divmod_core`
- `--runtime-profile small` also shares signed Q16.16 multiply/divide through unsigned Q16.16 helpers
- `--runtime-profile small` lowers `__rt_f32_sub` as a compact wrapper around `__rt_f32_add`
- `--runtime-profile balanced` keeps the standalone helper bodies; `fast` remains documented as balanced behavior for now
- `--size`, `--memory-report`, and `.map` include runtime helper word totals by category
- `--memory-report` shows selected helper variants and dependencies
- `.lst` helper comments show helper category and required-by metadata
- no arithmetic semantic changes, no `double`, no full math library, no recursion, and no new C syntax

Phase 32 scope:

- explicit page/layout summaries in `--size`, `--memory-report`, and `.map`
- inferred code-section metadata for vectors/startup, user functions, runtime helpers, dispatchers, ROM tables, and stack trap
- linker relaxation removes provably redundant same-page `setpage` pseudo-ops before encoding
- final Phase 31 page-safety validation still rejects unsafe `goto` / `call` edges after relaxation
- `.map` includes a `Code Layout` section with per-page usage and section placement
- no code reordering yet, no new C syntax, no `double`, no math library, and no recursion

Phase 31 scope:

- page-safe backend emission for unconditional `goto` and `call` targets
- centralized page-safe conditional branch helpers that do not rely on local stubs crossing page boundaries
- final backend layout validation for raw `goto` / `call` edges after labels and addresses are known
- `.map` code symbols include page metadata such as `page=1`
- `.lst` page-control pseudo-ops annotate page-safe control-flow targets
- simulator regressions cover Q16.16 helpers, float/ROM/function-pointer paths, and stack-check trap layout across multi-page programs
- no new C syntax, no `double`, no math library, no recursion, and no broad optimization work

Phase 30 scope:

- `float` as a 4-byte little-endian scalar with IEEE-754 single-precision storage bits
- decimal float literals such as `1.5` and `1.5f`
- float globals, statics, locals, parameters, returns, arrays, structs, unions, and data pointers
- finite-only constant folding for literals, unary minus, arithmetic, comparisons, and constant casts
- dynamic 16-bit integer and fixed-point casts through finite `f32 <-> Q16.16` bridge helpers
- dynamic `long` / `unsigned long` to `float` helpers
- dynamic `float` to `long` / `unsigned long` helpers with truncation toward zero
- dynamic float comparisons via shared `__rt_f32_cmp`
- dynamic float add/sub helpers that convert through an internal Q16.16 work format
- inline fast paths for common finite `* 2.0f` and `/ 2.0f`
- `const __rom float[]` calibration tables with omitted-size inference and direct `table[index]` reads
- constant and dynamic ROM float indexing through the existing 32-bit RETLW byte-read path
- ROM float payload layout as little-endian f32 bytes, for example `1.0f` emits `00 00 80 3F`
- simulator execution tests for raw literals, assignment, arithmetic, unary minus, folded/dynamic comparisons, dynamic casts, ROM float reads, calls, structs, arrays, pointers, and unions
- resource reports classify emitted float helpers as `float helper`
- resource reports and maps list ROM float tables as `ROM RETLW table` contributions
- no `double`, math library, recursion, general ROM pointers, or full IEEE special-value compliance

Phase 26 scope:

- explicit config-word descriptors for `PIC16F628A` and `PIC16F877A`
- symbolic `#pragma config FIELD = VALUE` support
- raw `__config(0x....)` fallback
- config word emitted in HEX and surfaced in `.map` / `.lst`
- always-on final HEX validation for target range, vectors, config, 14-bit words, checksums, and EOF
- `--verify-hex` detailed validation report
- `--print-program-command`, `--program`, and `--program-cmd <cmd>` for external programmer workflows
- Makefile `make size`, `make sim`, `make flash`, `FLASH_CMD`, and `FLASH_ARGS`
- hardware smoke examples under `examples/hardware/`
- no recursion or advanced optimization work

Phase 25 scope:

- explicit program/data memory descriptors for `PIC16F628A` and `PIC16F877A`
- target fit validation for encoded program words, config-word overlap, data RAM layout, and software-stack capacity
- clearer diagnostics for program memory overflow, data RAM overflow, stack region overflow, ROM table/code overlap, and descriptor mistakes
- user-facing `--size`, `--memory-report`, and `--memory-report-file <path>` options
- `.map` memory summary and `.lst` resource-summary comments
- runtime-helper contribution reporting, including large Q16.16 fixed-point helpers
- examples for small firmware size, fixed-helper cost, and ROM-table contribution
- no IEEE float, recursion, or advanced optimization work

Phase 24 scope:

- fixed-point scalar keywords `__fixed8_8`, `__ufixed8_8`, `__fixed16_16`, and `__ufixed16_16`
- little-endian raw storage: Q8.8/UQ8.8 are 2 bytes; Q16.16/UQ16.16 are 4 bytes
- raw fixed constructors `__q8_8(raw)`, `__uq8_8(raw)`, `__q16_16(raw)`, and `__uq16_16(raw)`
- fixed decimal literals such as `1.5q8_8`, `2.25uq8_8`, `10.125q16_16`, and `0.5uq16_16`
- fixed globals, statics, locals, parameters, returns, arrays, structs, unions, and data pointers
- Q8.8/UQ8.8 add, subtract, compare, multiply, divide, unary negate, and integer/fixed casts
- fixed-point ROM calibration arrays with direct indexing
- Q16.16/UQ16.16 add, subtract, compare, casts, storage, and ABI support
- Q16.16/UQ16.16 multiply/divide constants fold exactly
- dynamic Q16.16/UQ16.16 multiply/divide lower through page-safe runtime helpers
- Q8.8 fixed multiply/divide lower through proven Phase 21 32-bit helper paths; ISR helper restrictions still apply
- simulator execution tests for casts, fixed decimal literals, fixed ROM reads, Q8.8 arithmetic, calls, aggregates, arrays, UQ8.8 arithmetic, Q16.16 constant folding, and dynamic Q16.16 helpers
- conservative diagnostics for unsupported bitwise fixed ops, fixed modulo, fixed division by constant zero, malformed fixed literals, ISR helper use, and unsafe implicit fixed conversions
- no IEEE float, recursion, or advanced optimization work

Phase 21 scope:

- `long`, `signed long`, and `unsigned long` as 32-bit integer types
- little-endian 4-byte storage for globals, statics, locals, parameters, returns, arrays, structs, and unions
- literal suffixes `U`, `L`, `UL`, and `LU`
- 32-bit inline add/subtract, bitwise ops, comparisons, and shifts
- 32-bit runtime helpers for multiply, divide, modulo, and dynamic shifts
- Stack-first ABI extension for 4-byte arguments and returns: `W`, `return_high`, `return_upper0`, `return_upper1`
- simulator execution tests for 32-bit arithmetic, helpers, calls, aggregates, arrays, unions, and startup initialization
- conservative diagnostics for narrowing, oversized literals, helper use inside ISRs, and deferred 32-bit ROM objects
- no float, recursion, or advanced optimization work

Phase 20 scope:

- optional `pic16-sim` binary for running generated Intel HEX files
- `.map` loading for code/data symbol lookup
- `--run-until <symbol>` stop support, including generated labels such as `__halt`
- bounded execution through `--max-steps`
- RAM/SFR symbol printing and compact register dumps
- instruction tracing to stdout or a trace file
- user-friendly diagnostics for malformed HEX, invalid map files, unknown symbols, unsupported instructions, and step-limit exhaustion
- simulator-focused examples under `examples/sim/`
- no C language expansion; floats and recursion remain unsupported

Phase 19 scope remains:

- internal test-only PIC16 core emulator for the subset of 14-bit instructions emitted by the current backend
- execution tests that compile real C inputs to HEX, load them into the emulator, run to `__halt`, and assert final RAM/SFR results
- runtime validation coverage for arithmetic, control flow, stack/calls, function-pointer dispatch, pointers, aggregates, bitfields, and ROM reads
- explicit simulator errors for unsupported instructions or malformed HEX input
- execution validation stays modular under `src/sim/` and `tests/execution_sim.rs`

Phase 18 scope remains:

- target-aware software-stack bounds with `__stack_base`, `__stack_limit`, `__stack_ptr`, and `__frame_ptr`
- optional `--stack-check` runtime overflow guards on frame growth, helper calls, direct calls, and function-pointer dispatcher calls
- generated `__stack_overflow_trap` infinite-loop handler when runtime checks are enabled
- `--stack-report` / `--stack-report-file` per-function stack-usage visibility, ISR context accounting, and function-pointer target-set reporting
- stronger call-graph expansion across direct calls, helpers, ISR roots, and generated function-pointer dispatch groups
- stronger recursion diagnostics; recursion remains rejected in this phase

Phase 17 scope remains:

- function pointer object types with supported zero-arg/one-arg scalar signatures
- taking function addresses and assigning compatible function pointers
- direct calls through function pointers, arrays of function pointers, and function-pointer struct fields
- generated dispatch-ID trampolines instead of raw PIC16 computed calls
- explicit diagnostics for incompatible signatures, function-pointer arithmetic/relational comparisons, and ISR indirect-call restrictions

Phase 16 scope remains:

- fixed multidimensional RAM arrays with row-major layout
- repeated indexing like `matrix[i][j]`
- nested multidimensional initializer lists with zero-fill
- multidimensional array fields inside structs and unions
- chained designated initializers over mixed `.field` / `[index]` paths
- explicit diagnostics for multidimensional ROM arrays, incomplete multidimensional dimensions, and helper-requiring ISR index paths

Phase 15 scope remains:

- named `union` declarations for globals, locals, pointers, and nested struct fields
- union first-field and designated union initializers with whole-storage zero-fill
- whole-union assignment through the same byte-wise aggregate-copy path used for structs
- basic unsigned bitfields on `unsigned char` and `unsigned int`
- packed LSB-first bitfield layout within one storage unit, with real read-modify-write lowering
- explicit diagnostics for anonymous union fields, invalid bitfield widths/base types, and bitfield address-taking

Phase 14 scope remains:

- direct `rom_table[index]` reads for supported `const __rom` arrays
- `const __rom char[]`, `const __rom unsigned char[]`, `const __rom int[]`, and `const __rom unsigned int[]`
- `__rom_read8(table, index)` and `__rom_read16(table, index)` builtins
- constant-index ROM reads lowered inline when safe
- dynamic ROM reads lowered through RETLW-table dispatch
- richer ROM map/listing metadata and retained ROM/data-space pointer separation

Phase 12 scope remains:

- pointer-to-pointer types in the current data-space pointer model
- const-qualified pointer forms: pointer-to-const, const pointer, const pointer-to-const
- pointer relational comparisons for compatible data-space pointers
- pointer subtraction for compatible data-space pointers with 1-byte or 2-byte elements
- RAM-backed string literal objects that may initialize `char *` and `const char *`
- clearer pointer diagnostics for qualifier discard, invalid comparisons, and invalid subtraction

Phase 11 scope remains:

- arrays inside structs
- nested struct fields with composed offsets for `.` and `->`
- nested aggregate initializer lists with zero-fill
- designated initializers for `.field = value` and `[index] = value`
- string literal initialization for char/unsigned-char array fields
- whole-struct assignment for compatible complete struct types
- byte-wise struct-copy lowering through ordinary indirect load/store paths

Phase 10 scope remains:

- string literal lexing/parsing with `\n`, `\r`, `\t`, `\\`, `\"`, and `\0` escapes
- `char` / `unsigned char` arrays initialized from string literals
- omitted array-size inference from brace initializers and string literals
- RAM-backed `const` scalar/array/flat-struct objects with semantic write rejection
- startup-time initialization and zero-fill for globals, file-scope statics, and static locals
- clearer startup/listing/map output for initialized or zeroed static data

Phase 9 scope remains:

- `switch` statements over `char`, `unsigned char`, `int`, `unsigned int`, and enum-backed 16-bit `int`
- `case` labels with integer constant expressions or enum constants
- `default` labels
- `break` from switches
- ordinary C fallthrough between adjacent cases
- explicit diagnostics for duplicate or invalid switch labels

Phase 8 scope remains:

- `typedef` aliases for supported scalar/pointer/array/object forms
- `enum` declarations with implicit and explicit enumerator values
- named `struct` declarations with packed field layout metadata
- field access lowering for `.` and `->`
- array and struct positional initializer lists with zero-fill
- clearer explicit cast handling for scalar/pointer subsets
- explicit diagnostics for unsupported aggregate operations

What changed from Phase 3:

- all call arguments now use the software stack
- locals, local arrays, and IR temps are per-call frame storage
- 3+ argument calls are supported
- nested non-recursive calls use one coherent caller-pushed ABI
- `*`, `/`, `%`, `<<`, and `>>` now lower to real PIC16 code
- compiler runtime helper labels appear in `.map` and `.lst` when helper lowering is used
- `void __interrupt isr(void)` now emits a real interrupt vector at `0x0004`
- ISR code saves/restores CPU and ABI context conservatively, then returns with `retfie`
- Phase 6 ISR body rules reject normal calls and runtime-helper-requiring expressions
- Phase 7 reduces redundant instructions, shrinks temp pressure, and avoids some helper calls entirely
- Phase 8 adds typedef/enum/struct support, aggregate initializers, and explicit casts for supported forms
- Phase 9 adds compare-chain lowering for `switch` / `case` / `default`, fallthrough, and switch-aware `break`
- Phase 10 adds string literal parsing, RAM-backed const/static initialization cleanup, and clearer startup data artifacts
- Phase 11 adds nested aggregate layout/init support, designated initializers, and byte-wise whole-struct copy
- Phase 12 adds nested data pointers, const-qualified pointer forms, pointer compare/subtract, and RAM-backed string-literal pointer initialization
- Phase 13 introduces explicit `__rom` ROM arrays, RETLW-backed ROM tables, `__rom_read8()`, and separate ROM map/listing output
- Phase 14 adds direct ROM indexing, 16-bit ROM tables, `__rom_read16()`, constant-index inline ROM reads, and constant-only ISR ROM access
- Phase 15 adds named unions, union initializers/copy, and basic unsigned bitfield layout/read/write lowering
- Phase 16 adds row-major multidimensional RAM arrays, chained designators, and multidimensional aggregate field access
- Phase 17 adds controlled source-level function pointers, dispatch-ID lowering, and indirect-call diagnostics
- Phase 18 adds target-aware stack bounds, opt-in runtime overflow checks, and stack reports without enabling recursion
- Phase 19 adds emulator-based execution validation for generated PIC16 code without changing the supported C subset
- Phase 20 adds optional `pic16-sim` CLI debugging over generated HEX plus map-symbol inspection and tracing
- Phase 21 adds controlled 32-bit `long` / `unsigned long` storage, ABI, arithmetic, helpers, and simulator validation
- Phase 22 adds explicit fixed-point scalar types and simulator-validated Q8.8 arithmetic without enabling IEEE float
- Phase 23 adds fixed decimal literals, fixed ROM calibration tables, and Q16.16 constant folding
- Phase 24 adds dynamic Q16.16/UQ16.16 multiply/divide helpers without exposing public `long long`
- Phase 27 adds basic finite 32-bit software `float` storage, literals, constant folding, limited helpers, simulator validation, and resource reporting
- active docs now describe stack-first behavior; old Phase 2/3 docs remain historical

Historical milestone snapshots below describe what each phase introduced at the time. The current supported subset is summarized later under `Supported Subset`, `Current constraints`, and `Current Limits`.

## Phase 4 ABI Summary

- stack growth: upward
- argument order: left-to-right
- caller pushes argument bytes
- caller cleans argument bytes after return
- callee saves caller `FP`
- callee sets `FP` to the callee argument base
- callee allocates locals + IR temps above saved `FP`
- 8-bit return: `W`
- 16-bit return: low byte in `W`, high byte in backend helper slot `return_high`
- pointer return: same rule as 16-bit integer return

Frame layout with `FP` at callee argument base:

- `FP + 0 .. arg_bytes - 1`: argument bytes
- `FP + arg_bytes`: saved caller `FP` low
- `FP + arg_bytes + 1`: saved caller `FP` high
- `FP + arg_bytes + 2 ..`: locals, local arrays, IR temps

More detail:

- [DESIGN.md](DESIGN.md)
- [docs/backend/overview.md](docs/backend/overview.md)
- [docs/backend/phase4-stack-first-abi.md](docs/backend/phase4-stack-first-abi.md)
- [docs/backend/phase4-stack-model.md](docs/backend/phase4-stack-model.md)
- [docs/backend/phase5-helper-calling.md](docs/backend/phase5-helper-calling.md)
- [docs/backend/phase6-interrupts.md](docs/backend/phase6-interrupts.md)
- [docs/ir/phase4-call-lowering.md](docs/ir/phase4-call-lowering.md)
- [docs/ir/phase5-arithmetic-lowering.md](docs/ir/phase5-arithmetic-lowering.md)
- [docs/ir/phase6-isr-lowering.md](docs/ir/phase6-isr-lowering.md)
- [docs/ir/phase8-aggregate-lowering.md](docs/ir/phase8-aggregate-lowering.md)
- [docs/ir/phase9-switch-lowering.md](docs/ir/phase9-switch-lowering.md)
- [docs/ir/phase10-static-initializers.md](docs/ir/phase10-static-initializers.md)
- [docs/ir/phase11-aggregate-initializers.md](docs/ir/phase11-aggregate-initializers.md)
- [docs/frontend/phase6-isr-syntax.md](docs/frontend/phase6-isr-syntax.md)
- [docs/frontend/phase8-types.md](docs/frontend/phase8-types.md)
- [docs/frontend/phase9-switch.md](docs/frontend/phase9-switch.md)
- [docs/frontend/phase10-string-literals.md](docs/frontend/phase10-string-literals.md)
- [docs/frontend/phase11-aggregates.md](docs/frontend/phase11-aggregates.md)
- [docs/backend/phase8-struct-layout.md](docs/backend/phase8-struct-layout.md)
- [docs/backend/phase9-switch-codegen.md](docs/backend/phase9-switch-codegen.md)
- [docs/backend/phase10-data-layout.md](docs/backend/phase10-data-layout.md)
- [docs/backend/phase11-aggregate-copy.md](docs/backend/phase11-aggregate-copy.md)
- [docs/runtime/phase5-arithmetic-helpers.md](docs/runtime/phase5-arithmetic-helpers.md)
- [docs/migration/phase3-to-phase4-abi.md](docs/migration/phase3-to-phase4-abi.md)

## Phase 5 Arithmetic Summary

Supported integer operators:

- `*`, `/`, `%`, `<<`, `>>`
- `char`, `unsigned char`, `int`, `unsigned int`

Lowering strategy:

- constant folds and tiny identities stay inline
- constant shifts lower inline
- dynamic shifts and most multiply/divide/modulo paths lower through PIC16 runtime helpers
- helper calls use same stack-first ABI as normal functions

Current helper labels:

- `__rt_mul_u8`, `__rt_mul_i8`, `__rt_mul_u16`, `__rt_mul_i16`
- `__rt_div_u8`, `__rt_div_i8`, `__rt_div_u16`, `__rt_div_i16`
- `__rt_mod_u8`, `__rt_mod_i8`, `__rt_mod_u16`, `__rt_mod_i16`
- `__rt_shl8`, `__rt_shl16`, `__rt_shr_u8`, `__rt_shr_i8`, `__rt_shr_u16`, `__rt_shr_i16`

Behavior notes:

- unsigned right shift is logical
- signed right shift is arithmetic
- constant shift count `>=` bit width is rejected
- dynamic shift counts clamp to operand bit width inside helper lowering
- division or modulo by constant zero is rejected
- dynamic division/modulo by zero returns `0`
- arithmetic uses fixed-width PIC16-style wrap/truncation; 8-bit multiply returns low 8 bits

## Phase 6 Interrupt Summary

Chosen ISR syntax:

- `void __interrupt isr(void)`

Phase 6 interrupt model:

- one ISR per program
- ISR must return `void`
- ISR must take no parameters
- reset vector stays at `0x0000`
- interrupt vector is emitted at `0x0004`
- when an ISR exists, the vector dispatches to it through a nearby page-safe stub
- when no ISR exists, the default interrupt vector is `retfie`

Saved ISR context:

- `W`
- `STATUS`
- `PCLATH`
- `FSR`
- `return_high`
- `scratch0`
- `scratch1`
- `stack_ptr`
- `frame_ptr`

Phase 6 ISR restrictions:

- no normal function calls inside ISR
- no runtime-helper calls inside ISR
- any `*`, `/`, `%`, `<<`, `>>` expression that would lower through a Phase 5 helper is rejected
- inline-safe arithmetic, comparisons, assignments, SFR access, pointer dereference, and stack-backed locals remain allowed

Current ISR interaction model with the stack-first ABI:

- ISR uses the same frame machinery as normal functions
- ISR saves interrupted ABI state first
- ISR may use locals and IR temps on the software stack
- ISR restores interrupted ABI state before `retfie`

## Phase 7 Optimization Summary

Current optimization order for `-O1`, `-O2`, and `-Os`:

1. IR constant propagation and folding
2. IR dead code elimination
3. IR temp-slot compaction
4. backend helper fast paths and bank/page cleanup
5. backend peephole cleanup

Current Phase 7 wins:

- constant branches fold to direct jumps before backend lowering
- unreachable IR blocks are cleared before codegen
- unused temp ids are compacted to shrink frame pressure
- unsigned `/ 2^n` lowers to right shift instead of helper call
- unsigned `% 2^n` lowers to `andlw` mask instead of helper call
- redundant `movf x,w` + `movwf x`, duplicate `movwf`, duplicate bit ops, duplicate `setpage`, and overwritten W loads are removed
- `--opt-report` prints a compact optimization summary after a successful compile

Current integer-promotion subset:

- `*`, `/`, `%`, `&`, `|`, `^` balance both operands to one integer type
- same type stays unchanged
- integer literal adopts other operand type when possible
- otherwise wider width wins
- equal-width mixed signedness is rejected unless user adds an explicit cast
- shift result type is left operand type; right operand is coerced to left operand type

## Phase 8 Type-System Summary

`typedef`:

- file-scope typedef aliases are supported for scalar, pointer, array, and named struct object forms
- duplicate typedef names are rejected
- typedef/object-function name conflicts are rejected in this phase

`enum`:

- implicit enumerators start at `0` and increment by `1`
- explicit enumerator constants are supported
- enum constants are compile-time integer constants usable in expressions
- enum representation is fixed to 16-bit `int` in this phase

`struct`:

- named structs are supported
- layout is packed declaration order with no inserted padding
- field offsets are byte offsets from base address
- `.` and `->` lower through base-pointer + constant-offset addressing
- whole-struct copy assignment is rejected in this phase

Initializers:

- scalar expression initializers remain supported
- array and struct positional initializer lists are supported
- missing aggregate elements are zero-filled
- too many initializer elements are diagnosed
- designated initializers are currently rejected with an explicit diagnostic

Explicit casts:

- scalar widening/narrowing and signedness casts are supported
- explicit narrowing suppresses implicit narrowing warnings
- casts between supported data pointers are supported
- integer-to-pointer is restricted to integer zero (`(T*)0`)
- pointer-to-integer is restricted to 16-bit integer targets

## Phase 9 Switch Summary

Supported:

- `switch (expr)` over `char`, `unsigned char`, `int`, `unsigned int`, and enum-backed 16-bit `int`
- `case` labels using integer constant expressions or enum constants
- one optional `default` label per switch
- `break` exits the innermost enclosing switch; in mixed loop/switch nesting it does not exit an outer loop
- fallthrough between adjacent cases when no `break`, `return`, or other control transfer intervenes
- nested switches
- switches inside loops and loops inside switches
- compare-and-branch lowering; no jump tables in this phase

Diagnostics:

- duplicate case values
- multiple `default` labels
- `case` / `default` outside switches
- non-constant case labels
- case values not representable in the switch expression type
- unsupported non-integer switch expressions
- case/default labels nested under other control statements in the same switch are rejected in this phase

## Phase 10 Static-Data Summary

Supported:

- string literals parse as null-terminated byte strings
- supported escapes: `\n`, `\r`, `\t`, `\\`, `\"`, `\0`
- `char` and `unsigned char` arrays may initialize from string literals
- omitted array size may be inferred from brace initializer element count or string length plus trailing null
- globals, file-scope statics, and static locals are zero-initialized when no initializer is present
- scalar, array, and flat-struct globals/statics initialize through startup code
- missing array/struct initializer elements are zero-filled
- `const` scalar, array, and flat-struct objects are read-only at semantic level
- map entries annotate const/static data; startup/listing output annotates zero/init actions

Diagnostics:

- unterminated string literals
- unsupported string escape sequences
- string initializers that do not fit including the trailing null byte
- string literals used with incompatible scalar or pointer targets
- non-constant global/static initializer forms
- assignment to const objects

## Phase 11 Aggregate Summary

Supported:

- arrays inside structs with packed declaration-order layout
- nested struct fields with composed constant offsets
- nested aggregate initializer lists for arrays and structs
- string literal initialization of `char` / `unsigned char` array fields
- zero-fill for omitted nested array/struct elements
- designated struct-field initializers: `.field = value`
- designated array-index initializers: `[index] = value`
- whole-struct assignment between compatible complete struct types

Diagnostics:

- duplicate designated fields
- duplicate array designators
- unknown designated fields
- array designator indices that are non-constant or out of range
- too many initializer elements for arrays or structs
- self-containing structs by value
- incompatible struct assignment
- assignment to const aggregate objects
- whole-struct assignment inside interrupt handlers

## Phase 12 Pointer Summary

Supported:

- nested data-space pointer types including pointer-to-pointer values, parameters, and returns
- const-qualified pointer forms: `const T *`, `T * const`, and `const T * const`
- implicit conversion from `T *` to `const T *`
- pointer equality/inequality and relational comparisons for compatible data-space pointer types
- pointer subtraction for compatible data-space pointer types with 1-byte or 2-byte elements
- RAM-backed string literal objects that may initialize `char *` and `const char *`
- startup/listing comments that name string-literal symbols and pointer-valued static initializers
- map output that groups string-literal data symbols explicitly

Diagnostics:

- incompatible pointer assignments
- qualifier discard through pointers
- writes through pointer-to-const
- reassignment of const pointer objects
- incompatible pointer relational comparisons
- unsupported pointer subtraction element sizes
- incompatible string-literal pointer targets

## Historical Phase 14 ROM Summary

Supported:

- file-scope `const __rom unsigned char[]`, `const __rom char[]`, `const __rom unsigned int[]`, and `const __rom int[]`
- brace-list ROM table initializers
- string-literal ROM string initializers
- direct `table[index]` reads over named ROM arrays of supported element types
- `__rom_read8(table, index)` over ROM byte-array objects
- `__rom_read16(table, index)` over ROM 16-bit array objects
- `.hex`, `.map`, and `.lst` output that shows ROM tables separately
- RETLW-backed ROM table lowering with one entry instruction plus one program word per data byte
- little-endian byte packing for 16-bit ROM table elements
- constant-index ROM reads lowered inline without dynamic table dispatch
- constant-index ROM reads inside ISR when the expression stays inline-safe

Diagnostics:

- non-const ROM objects
- local ROM objects
- writes to ROM array elements
- taking the address of ROM array elements
- ROM/data-pointer mixing
- ROM pointer types
- dynamic ROM reads inside interrupt handlers
- unsupported ROM object types
- oversize ROM tables that do not fit one Phase 14 RETLW page

## Supported Subset

Supported:

- `#include`
- object-like `#define`
- `#if`, `#ifdef`, `#ifndef`, `#else`, `#endif`
- functions
- globals
- auto locals
- static locals
- file-scope `typedef` aliases for supported object/value types
- `enum` declarations and enumerator constants
- named packed `struct` declarations with nested struct, named union, fixed-size array, and basic unsigned bitfield fields
- named packed `union` declarations with supported scalar, pointer, array, struct, or union members
- file-scope `const __rom char[]`, `const __rom unsigned char[]`, `const __rom int[]`, and `const __rom unsigned int[]`
- `if` / `else`
- `while`
- `for`
- `do while`
- `switch` / `case` / `default`
- `break` / `continue`
- `return`
- direct calls
- `char`, `unsigned char`, `int`, `unsigned int`, `void`
- fixed-size arrays of supported scalar types and complete named struct/union types
- omitted array size when inferred from a brace initializer list or string literal
- complete named struct objects with scalar, fixed-size array, nested struct, named union, bitfield, or supported pointer fields
- complete named union objects with supported scalar, pointer, array, struct, or union fields
- `const` scalar, one-dimensional array, and complete named struct/union objects
- nested data-space pointers to supported scalar, pointer, or complete named struct/union types in PIC16 RAM
- controlled source-level function pointers with supported scalar signatures
- `&obj`
- `*ptr`
- `a[i]`
- `p[i]`
- repeated array indexing like `matrix[i][j]`
- `.` and `->`
- basic unsigned bitfield member access and assignment
- unary `!`, `~`, unary `-`
- `+`, `-`, `*`, `/`, `%`, `<<`, `>>`, `&`, `|`, `^`
- `==`, `!=`, `<`, `<=`, `>`, `>=`
- compile-time `sizeof` for supported scalars, pointers, and fixed arrays
- direct `rom_table[index]` reads for supported ROM arrays
- `__rom_read8(table, index)` for program-memory byte arrays
- `__rom_read16(table, index)` for program-memory 16-bit arrays
- calls through compatible function-pointer objects, arrays, and struct fields
- positional and designated array/struct/union initializer lists with zero-fill
- chained designated initializer paths such as `.a.x`, `[1][2]`, and `.field[1][2]`
- nested aggregate initializer lists
- string literal initialization for char/unsigned-char array fields and char/unsigned-char array fields inside structs
- RAM-backed string literal initialization of `char *` and `const char *`
- whole-struct and whole-union assignment between compatible complete types
- explicit casts for supported scalar and data-pointer forms
- indirect data access through `FSR/INDF`
- 3+ argument stack calls
- stack-backed local arrays
- pointer arguments and pointer returns
- function-pointer arguments and returns as ordinary 16-bit scalar values
- one `void __interrupt isr(void)` handler
- interrupt vector emission at `0x0004`
- `retfie`

Partially supported / constrained:

- `typedef` names are file-scope only and cannot conflict with object/function names
- enums use fixed 16-bit `int` representation in this phase
- structs use packed declaration-order layout with no implicit padding
- unions use packed max-field-size layout with every field at byte offset `0`
- struct/union copy lowers byte-by-byte through ordinary indirect memory operations
- bitfields are limited to `unsigned char` and `unsigned int`, pack LSB-first within one storage unit, and reject address-taking
- multidimensional arrays use row-major layout and support fixed explicit RAM dimensions only
- designated initializers support chained `.field` / `[index]` paths over complete array/struct/union subobjects
- global aggregate initializer elements must be constant expressions
- explicit casts are limited to scalar conversions, data-pointer bitcasts, `(T*)0`, and pointer-to-16-bit-integer casts
- string literals become RAM-backed static data objects when used in pointer-initializer contexts; duplicate pooling is not attempted
- startup code writes initialized global/static bytes and clears zero-init storage; explicit `__rom` objects bypass startup and emit into program memory as RETLW tables
- plain `const` objects remain RAM-backed in this phase unless explicitly declared `__rom`
- direct ROM indexing and `__rom_read8()` / `__rom_read16()` read named ROM arrays through inline constants or generated RETLW table calls rather than a general code-space pointer model
- nested pointer qualifier conversions are intentionally conservative: exact nested qualifiers are required beyond one-level `T *` to `const T *`
- pointer subtraction assumes the compared pointers refer into the same object, matching ordinary C same-object expectations
- pointer subtraction supports only element sizes of 1 or 2 bytes in this phase
- multidimensional arrays do not decay to data pointers in this phase; use direct indexing instead of passing them as pointer values
- multidimensional array parameter types remain rejected rather than introducing an incorrect pointer-to-array ABI model
- local aggregate initializers and whole-aggregate copies remain rejected inside interrupt handlers
- ROM objects are limited to file-scope 8-bit/16-bit integer arrays whose byte payload fits one 255-byte RETLW table page
- multidimensional ROM arrays remain deferred; `__rom` currently stays one-dimensional only
- function pointers lower through generated dispatch IDs plus per-signature dispatcher code; raw PIC16 computed calls remain intentionally deferred
- supported indirect-call signatures are limited to zero or one integer argument with `void`, `char`, `unsigned char`, `int`, or `unsigned int` return values
- function-pointer values are 16-bit dispatch IDs; null is `0`, and generated dispatch labels in `.map` / `.lst` show the assigned IDs
- switch lowering uses linear compare chains; no jump tables are emitted in this phase
- case/default labels must stay in the switch body flow or nested blocks; labels under unrelated control statements like `if`, `while`, or `for` are rejected in phase 9

Unsupported:

- anonymous nested struct/union/enum fields without declarators
- signed bitfields
- pointer-to-function-pointer object models
- function-pointer arithmetic or relational comparisons
- function-pointer calls inside interrupt handlers
- ROM tables of function pointers / ROM function addresses
- pointers to incomplete struct/union types
- program-memory / code-space pointers
- `float`
- recursion

Current constraints:

- recursion is still rejected in Phase 18; `--stack-check` adds runtime bounds checks for bounded acyclic call trees but does not enable recursive cycles
- returning a pointer to stack-local storage is rejected, including direct forms and obvious local alias chains
- explicit casts stay limited to scalar conversions, data-pointer bitcasts, `(T*)0`, and pointer-to-16-bit-integer casts
- aggregate initializers inside interrupt handlers remain rejected
- whole-aggregate copy inside interrupt handlers remains rejected
- global aggregate initializer elements must be constant expressions
- string literal array initializers are accepted only for `char` / `unsigned char` arrays, and explicit array sizes must fit the trailing null byte too
- string literals that initialize pointers become anonymous RAM-backed static objects; map output groups them under string literals
- globals, file-scope statics, and static locals are initialized by startup code in RAM, not by a separate read-only data segment
- plain const RAM data remains in startup-initialized RAM; explicit `__rom` data uses separate RETLW-backed program-memory tables
- nested data-space pointer comparisons are raw address-order comparisons in RAM
- implicit nested-pointer qualifier changes beyond `T *` to `const T *` are rejected conservatively
- pointer subtraction is limited to compatible pointer types whose element size is 1 or 2 bytes
- ROM reads use direct indexing plus `__rom_read8()` / `__rom_read16()` only; general ROM address values and ROM pointers are still unsupported
- function pointers use generated compare-chain dispatchers; raw computed PIC16 indirect calls are not used
- function-pointer calls and function-pointer table dispatch remain forbidden inside ISR code
- pointer-to-function-pointer declarations remain rejected conservatively in this phase
- software stack grows upward from target-specific `stack_base` toward exclusive `stack_limit`; map output exposes both bounds plus `__stack_ptr` and `__frame_ptr`
- `--stack-check` emits inline overflow guards before frame growth and argument-push growth; overflow branches to `__stack_overflow_trap`, an infinite loop
- `--stack-report` prints per-function frame/helper/call-depth data; `--stack-report-file <path>` writes same detailed report to disk
- stack reports expand function-pointer indirect calls across every known target in the matching signature group and mark unknown target sets conservatively
- multidimensional arrays are RAM-only in this phase; multidimensional `__rom` arrays are rejected explicitly
- multidimensional arrays do not decay to pointers; direct indexing like `matrix[i][j]` is the supported access path
- multidimensional array parameter types remain unsupported
- unions are named only; anonymous union fields remain deferred
- bitfields support only unsigned base types, pack LSB-first within one storage unit, and reject `&field`
- ROM unions and ROM bitfield objects are still unsupported
- switch expressions must stay in the supported integer subset; case labels must be constant and representable
- switch inside ISR is allowed only when the controlling expression and body remain inline-safe under existing Phase 6 helper restrictions
- reads from global/static/const RAM objects are allowed inside ISR when the resulting expressions stay inline-safe
- pointers are data-space-only; code pointers remain unsupported
- only one ISR is supported in this phase
- ISR code cannot call normal functions or Phase 5 runtime helpers
- no emulator or hardware execution runs in CI; validation is compile/listing/map/HEX shape based

## Known Limitations (Historical Phase 14 Snapshot)

- in that historical snapshot, recursion was still unsupported
- in that historical snapshot, no runtime software-stack overflow detection was implemented
- in that historical snapshot, ISR restrictions remained conservative: one ISR, no normal calls, no runtime-helper-requiring expressions
- in that historical snapshot, aggregate support was still intentionally constrained: no multidimensional arrays, no anonymous nested fields, no chained designators, and no incomplete-struct pointers
- in that historical snapshot, switch lowering stayed intentionally simple: compare chains only, no jump tables, and labels nested under other control statements were rejected
- in that historical snapshot, automatic string literals used as pointers were still RAM-backed static data objects; there was no automatic ROM string pooling pass
- in that historical snapshot, only explicit `const __rom` arrays used program memory; there was still no general ROM pointer or code-space string-pointer model
- in that historical snapshot, implicit nested-pointer qualifier conversions stayed conservative beyond one-level `T *` to `const T *`
- in that historical snapshot, enums stayed fixed to 16-bit `int`; structs stayed packed with no padding
- in that historical snapshot, `union`, source-level function pointers, and multidimensional arrays were still unsupported
- dynamic division/modulo by zero returns `0` (constant zero divisors are diagnostics)
- pointers are data-space only; code pointers are unsupported
- pointer subtraction is intentionally limited to 1-byte and 2-byte element types

## Build

Requirements:

- Linux
- recent stable Rust

Commands:

```bash
cargo check
cargo test
cargo clippy --all-targets -- -D warnings
```

Phase 20 simulator CLI tests live in `tests/sim_cli.rs`.
Phase 19 execution tests live in `tests/execution_sim.rs` and use the internal emulator in `src/sim/mod.rs`.

## Simulator CLI

Phase 20 exposes the internal emulator through `pic16-sim`.

Example:

```bash
cargo build --release
./target/release/picc --target pic16f877a -I include --map --list-file -o build/arithmetic.hex examples/sim/arithmetic_sim.c
./target/release/pic16-sim build/arithmetic.hex --map build/arithmetic.map --run-until __halt --print-symbol result
```

Expected output:

```text
result = 5 (0x05)
```

Useful options:

- `--run-until <symbol>` stops before executing a code symbol from the `.map`
- `--max-steps <n>` bounds execution
- `--print-symbol <name>` prints one RAM/SFR value
- `--print-regs` prints PC, W, STATUS, PCLATH, FSR, step count, and return-stack top
- `--trace` prints compact instruction trace lines
- `--trace-file <path>` writes trace output to a file
- `--target <name>` selects `pic16f628a` or `pic16f877a`; default is `pic16f877a`

See:

- [docs/sim/pic16-sim-cli.md](docs/sim/pic16-sim-cli.md)
- [docs/developer-guide/simulator-workflow.md](docs/developer-guide/simulator-workflow.md)

## Execution Validation

Phase 19 added a small internal PIC16 core emulator for compiler regression tests.

- normal `picc` CLI behavior is unchanged; simulation is optional tooling
- tests compile C to Intel HEX, load the HEX image, run until a generated stop label such as `__halt`, and then inspect RAM/SFR state
- supported runtime coverage includes arithmetic, branches/loops, stack-first calls, helper calls, function-pointer dispatch, pointers, aggregates, bitfields, multidimensional arrays, and explicit `__rom` table reads
- unsupported instruction words fail clearly in the simulator instead of silently executing bad behavior

See:

- [docs/testing/phase19-emulator.md](docs/testing/phase19-emulator.md)
- [docs/sim/pic16-core-emulator.md](docs/sim/pic16-core-emulator.md)
- [docs/developer-guide/testing.md](docs/developer-guide/testing.md)

## Installing picc

```bash
cargo build --release
./target/release/picc --version
./target/release/picc --help
./target/release/pic16-sim --version
./target/release/pic16-sim --help

cargo install --path .
picc --version
picc --help
pic16-sim --version
pic16-sim --help

# Verify PATH precedence to avoid stale binary confusion
which -a picc
which -a pic16-sim
command -v picc
```

If multiple `picc` paths are listed, the first one in `which -a picc` is the binary the shell executes.
After `cargo install --path .`, the expected installed path is typically `$HOME/.cargo/bin/picc`.

## Compiling to HEX

```bash
picc --target pic16f877a -Wall -Wextra -Werror -O2 -I include -o build/main.hex src/main.c
```

## Using picc from Makefile

```make
PIC := picc
TARGET := pic16f877a
CFLAGS := -Wall -Wextra -Werror -O2 -I include
SRC := src/main.c
OUT := build/main.hex
FLASH_CMD ?= echo "Configure FLASH_CMD to program"

$(OUT): $(SRC)
	mkdir -p build
	$(PIC) --target $(TARGET) $(CFLAGS) -o $(OUT) $(SRC)

clean:
	rm -rf build

flash: $(OUT)
	$(FLASH_CMD) $(OUT)
```

Variables:

- `PIC`: compiler executable
- `TARGET`: PIC device name
- `CFLAGS`: compiler flags and include paths
- `SRC`: input C file
- `OUT`: output HEX path
- `FLASH_CMD`: programmer command (override per toolchain)

Commands:

- `make`
- `make clean`
- `make flash`

Override `PIC` to force a specific binary (useful when validating a local release build):

```bash
cd examples/pic16f628a
make clean
make PIC=../../target/release/picc
```

## Narrowing Conversion Diagnostics

Current narrowing policy in semantic analysis:

- representable integer constant expressions may narrow without truncation diagnostics
- representable constants assigned to volatile byte SFRs do not warn
- out-of-range constants still trigger truncation diagnostics (`W1001`)
- non-constant narrowing conversions that may truncate still trigger diagnostics
- with `-Wall -Wextra -Werror`, these diagnostics become hard errors

Examples that are accepted under `-Werror` when values fit:

- `unsigned char i = 8;`
- `TRISB = 0x00;`
- `PORTB = 0x01;`

Examples that are rejected under `-Werror`:

- `unsigned char x = 300;`
- `PORTB = 300;`
- `int x; unsigned char y = x;`

## Usage

```bash
picc \
  --target pic16f628a \
  -I include \
  -O2 -Wall -Wextra \
  --opt-report \
  --emit-ast --emit-ir --emit-asm \
  --map --list-file \
  -o build/blink.hex \
  examples/pic16f628a/blink.c
```

Phase 4 stack-first example:

```bash
picc \
  --target pic16f628a \
  -I include \
  -O2 -Wall -Wextra \
  --emit-ir --emit-asm --map --list-file \
  -o build/stack-abi.hex \
  examples/pic16f628a/stack_abi.c
```

Phase 5 arithmetic-helper example:

```bash
picc \
  --target pic16f877a \
  -I include \
  -O2 -Wall -Wextra \
  --emit-ir --emit-asm --map --list-file \
  -o build/expression-test.hex \
  examples/pic16f877a/expression_test.c
```

Phase 6 interrupt example:

```bash
picc \
  --target pic16f628a \
  -I include \
  -O2 -Wall -Wextra \
  --emit-ir --emit-asm --map --list-file \
  -o build/timer-interrupt.hex \
  examples/pic16f628a/timer_interrupt.c
```

List targets:

```bash
picc --list-targets
```

## Examples

- [examples/pic16f628a/blink.c](examples/pic16f628a/blink.c)
- [examples/pic16f628a/arith16.c](examples/pic16f628a/arith16.c)
- [examples/pic16f628a/array_fill.c](examples/pic16f628a/array_fill.c)
- [examples/pic16f628a/array_initializer.c](examples/pic16f628a/array_initializer.c)
- [examples/pic16f628a/casts.c](examples/pic16f628a/casts.c)
- [examples/pic16f628a/function_pointer_basic.c](examples/pic16f628a/function_pointer_basic.c)
- [examples/pic16f628a/fixed_literals.c](examples/pic16f628a/fixed_literals.c)
- [examples/pic16f628a/fixed_q16_16_dynamic.c](examples/pic16f628a/fixed_q16_16_dynamic.c)
- [examples/pic16f628a/pointer_to_pointer.c](examples/pic16f628a/pointer_to_pointer.c)
- [examples/pic16f628a/rom_index.c](examples/pic16f628a/rom_index.c)
- [examples/pic16f628a/rom_table.c](examples/pic16f628a/rom_table.c)
- [examples/pic16f628a/stack_abi.c](examples/pic16f628a/stack_abi.c)
- [examples/pic16f628a/stack_report.c](examples/pic16f628a/stack_report.c)
- [examples/pic16f628a/string_array.c](examples/pic16f628a/string_array.c)
- [examples/pic16f628a/struct_array_field.c](examples/pic16f628a/struct_array_field.c)
- [examples/pic16f628a/struct_initializer.c](examples/pic16f628a/struct_initializer.c)
- [examples/pic16f628a/struct_point.c](examples/pic16f628a/struct_point.c)
- [examples/pic16f628a/switch_state.c](examples/pic16f628a/switch_state.c)
- [examples/pic16f628a/typedef_enum.c](examples/pic16f628a/typedef_enum.c)
- [examples/pic16f628a/union_basic.c](examples/pic16f628a/union_basic.c)
- [examples/pic16f877a/blink.c](examples/pic16f877a/blink.c)
- [examples/pic16f877a/call_chain.c](examples/pic16f877a/call_chain.c)
- [examples/pic16f877a/compare16.c](examples/pic16f877a/compare16.c)
- [examples/pic16f877a/const_pointers.c](examples/pic16f877a/const_pointers.c)
- [examples/pic16f877a/config_table.c](examples/pic16f877a/config_table.c)
- [examples/pic16f877a/div16.c](examples/pic16f877a/div16.c)
- [examples/pic16f877a/designated_init.c](examples/pic16f877a/designated_init.c)
- [examples/pic16f877a/expression_test.c](examples/pic16f877a/expression_test.c)
- [examples/pic16f877a/function_pointer_struct.c](examples/pic16f877a/function_pointer_struct.c)
- [examples/pic16f877a/function_pointer_table.c](examples/pic16f877a/function_pointer_table.c)
- [examples/pic16f877a/fixed_q16_16_arithmetic.c](examples/pic16f877a/fixed_q16_16_arithmetic.c)
- [examples/pic16f877a/fixed_q16_16_muldiv.c](examples/pic16f877a/fixed_q16_16_muldiv.c)
- [examples/pic16f877a/fixed_q16_16_sensor_scale.c](examples/pic16f877a/fixed_q16_16_sensor_scale.c)
- [examples/pic16f877a/fixed_rom_table.c](examples/pic16f877a/fixed_rom_table.c)
- [examples/pic16f877a/fixed_calibration.c](examples/pic16f877a/fixed_calibration.c)
- [examples/pic16f877a/fixed_conversion.c](examples/pic16f877a/fixed_conversion.c)
- [examples/pic16f877a/bitfield_flags.c](examples/pic16f877a/bitfield_flags.c)
- [examples/pic16f877a/bitfield_register_like.c](examples/pic16f877a/bitfield_register_like.c)
- [examples/pic16f877a/const_config.c](examples/pic16f877a/const_config.c)
- [examples/pic16f877a/global_init.c](examples/pic16f877a/global_init.c)
- [examples/pic16f877a/mod16.c](examples/pic16f877a/mod16.c)
- [examples/pic16f877a/mul16.c](examples/pic16f877a/mul16.c)
- [examples/pic16f877a/nested_struct.c](examples/pic16f877a/nested_struct.c)
- [examples/pic16f877a/pointer16.c](examples/pic16f877a/pointer16.c)
- [examples/pic16f877a/pointer_compare.c](examples/pic16f877a/pointer_compare.c)
- [examples/pic16f877a/pointer_subtract.c](examples/pic16f877a/pointer_subtract.c)
- [examples/pic16f877a/recursive_checked.c](examples/pic16f877a/recursive_checked.c)
- [examples/pic16f877a/rom_lookup_direct.c](examples/pic16f877a/rom_lookup_direct.c)
- [examples/pic16f877a/rom_lookup.c](examples/pic16f877a/rom_lookup.c)
- [examples/pic16f877a/rom_string.c](examples/pic16f877a/rom_string.c)
- [examples/pic16f877a/rom_string_index.c](examples/pic16f877a/rom_string_index.c)
- [examples/pic16f877a/rom_table16.c](examples/pic16f877a/rom_table16.c)
- [examples/pic16f877a/shift_mix.c](examples/pic16f877a/shift_mix.c)
- [examples/pic16f877a/state_dispatch_fp.c](examples/pic16f877a/state_dispatch_fp.c)
- [examples/pic16f877a/static_table.c](examples/pic16f877a/static_table.c)
- [examples/pic16f877a/stack_check.c](examples/pic16f877a/stack_check.c)
- [examples/pic16f877a/struct_copy.c](examples/pic16f877a/struct_copy.c)
- [examples/pic16f877a/string_pointer.c](examples/pic16f877a/string_pointer.c)
- [examples/pic16f877a/switch_enum.c](examples/pic16f877a/switch_enum.c)
- [examples/pic16f877a/switch_fallthrough.c](examples/pic16f877a/switch_fallthrough.c)
- [examples/pic16f877a/union_initializer.c](examples/pic16f877a/union_initializer.c)
- [examples/pic16f877a/union_struct_nested.c](examples/pic16f877a/union_struct_nested.c)
- [examples/pic16f877a/mutual_recursion_diagnostic.c](examples/pic16f877a/mutual_recursion_diagnostic.c)
- [examples/pic16f628a/timer_interrupt.c](examples/pic16f628a/timer_interrupt.c)
- [examples/pic16f877a/timer_interrupt.c](examples/pic16f877a/timer_interrupt.c)
- [examples/pic16f877a/gpio_interrupt.c](examples/pic16f877a/gpio_interrupt.c)

## Documentation

- [DESIGN.md](DESIGN.md)
- [CONTRIBUTING.md](CONTRIBUTING.md)
- [docs/architecture/overview.md](docs/architecture/overview.md)
- [docs/backend/overview.md](docs/backend/overview.md)
- [docs/backend/optimization.md](docs/backend/optimization.md)
- [docs/backend/phase4-stack-first-abi.md](docs/backend/phase4-stack-first-abi.md)
- [docs/backend/phase4-stack-model.md](docs/backend/phase4-stack-model.md)
- [docs/backend/phase10-data-layout.md](docs/backend/phase10-data-layout.md)
- [docs/backend/phase11-aggregate-copy.md](docs/backend/phase11-aggregate-copy.md)
- [docs/backend/phase12-string-pointer-data.md](docs/backend/phase12-string-pointer-data.md)
- [docs/backend/phase13-rom-data-layout.md](docs/backend/phase13-rom-data-layout.md)
- [docs/backend/phase14-retlw-tables.md](docs/backend/phase14-retlw-tables.md)
- [docs/backend/phase15-bitfield-codegen.md](docs/backend/phase15-bitfield-codegen.md)
- [docs/backend/phase16-aggregate-layout.md](docs/backend/phase16-aggregate-layout.md)
- [docs/backend/phase17-dispatcher.md](docs/backend/phase17-dispatcher.md)
- [docs/backend/phase18-stack-safety.md](docs/backend/phase18-stack-safety.md)
- [docs/backend/phase23-fixed-rom-tables.md](docs/backend/phase23-fixed-rom-tables.md)
- [docs/backend/phase9-switch-codegen.md](docs/backend/phase9-switch-codegen.md)
- [docs/developer-guide/testing.md](docs/developer-guide/testing.md)
- [docs/ir/phase4-call-lowering.md](docs/ir/phase4-call-lowering.md)
- [docs/backend/phase5-helper-calling.md](docs/backend/phase5-helper-calling.md)
- [docs/ir/phase5-arithmetic-lowering.md](docs/ir/phase5-arithmetic-lowering.md)
- [docs/ir/phase10-static-initializers.md](docs/ir/phase10-static-initializers.md)
- [docs/ir/phase11-aggregate-initializers.md](docs/ir/phase11-aggregate-initializers.md)
- [docs/ir/phase12-pointer-lowering.md](docs/ir/phase12-pointer-lowering.md)
- [docs/ir/phase13-rom-lowering.md](docs/ir/phase13-rom-lowering.md)
- [docs/ir/phase14-rom-read-lowering.md](docs/ir/phase14-rom-read-lowering.md)
- [docs/ir/phase15-aggregate-lowering.md](docs/ir/phase15-aggregate-lowering.md)
- [docs/ir/phase16-aggregate-index-lowering.md](docs/ir/phase16-aggregate-index-lowering.md)
- [docs/ir/phase17-indirect-call-lowering.md](docs/ir/phase17-indirect-call-lowering.md)
- [docs/ir/phase18-call-graph.md](docs/ir/phase18-call-graph.md)
- [docs/ir/phase23-fixed-constant-folding.md](docs/ir/phase23-fixed-constant-folding.md)
- [docs/ir/phase24-fixed-dynamic-lowering.md](docs/ir/phase24-fixed-dynamic-lowering.md)
- [docs/ir/phase9-switch-lowering.md](docs/ir/phase9-switch-lowering.md)
- [docs/frontend/phase10-string-literals.md](docs/frontend/phase10-string-literals.md)
- [docs/frontend/phase11-aggregates.md](docs/frontend/phase11-aggregates.md)
- [docs/frontend/phase12-pointers.md](docs/frontend/phase12-pointers.md)
- [docs/frontend/phase13-rom-address-space.md](docs/frontend/phase13-rom-address-space.md)
- [docs/frontend/phase14-rom-indexing.md](docs/frontend/phase14-rom-indexing.md)
- [docs/frontend/phase15-union-bitfields.md](docs/frontend/phase15-union-bitfields.md)
- [docs/frontend/phase16-multidimensional-arrays.md](docs/frontend/phase16-multidimensional-arrays.md)
- [docs/frontend/phase17-function-pointers.md](docs/frontend/phase17-function-pointers.md)
- [docs/frontend/phase23-fixed-literals.md](docs/frontend/phase23-fixed-literals.md)
- [docs/frontend/phase9-switch.md](docs/frontend/phase9-switch.md)
- [docs/developer-guide/stack-report.md](docs/developer-guide/stack-report.md)
- [docs/testing/phase19-emulator.md](docs/testing/phase19-emulator.md)
- [docs/sim/pic16-core-emulator.md](docs/sim/pic16-core-emulator.md)
- [docs/runtime/phase5-arithmetic-helpers.md](docs/runtime/phase5-arithmetic-helpers.md)
- [docs/runtime/phase23-q16-16-helpers.md](docs/runtime/phase23-q16-16-helpers.md)
- [docs/runtime/phase24-q16-16-dynamic-helpers.md](docs/runtime/phase24-q16-16-dynamic-helpers.md)
- [docs/frontend/phase27-float.md](docs/frontend/phase27-float.md)
- [docs/runtime/phase27-float-helpers.md](docs/runtime/phase27-float-helpers.md)
- [docs/frontend/phase28-float-conversions.md](docs/frontend/phase28-float-conversions.md)
- [docs/ir/phase28-float-dynamic-lowering.md](docs/ir/phase28-float-dynamic-lowering.md)
- [docs/runtime/phase28-float-runtime.md](docs/runtime/phase28-float-runtime.md)
- [docs/backend/phase28-float-resource-cost.md](docs/backend/phase28-float-resource-cost.md)
- [docs/runtime/phase29-float-compare.md](docs/runtime/phase29-float-compare.md)
- [docs/runtime/phase29-float-i32-conversions.md](docs/runtime/phase29-float-i32-conversions.md)
- [docs/ir/phase29-float-comparison-lowering.md](docs/ir/phase29-float-comparison-lowering.md)
- [docs/frontend/phase30-rom-float.md](docs/frontend/phase30-rom-float.md)
- [docs/ir/phase30-rom-float-lowering.md](docs/ir/phase30-rom-float-lowering.md)
- [docs/backend/phase30-rom-float-tables.md](docs/backend/phase30-rom-float-tables.md)
- [docs/runtime/phase34-shared-helper-primitives.md](docs/runtime/phase34-shared-helper-primitives.md)
- [docs/runtime/phase34-small-profile.md](docs/runtime/phase34-small-profile.md)
- [docs/runtime/phase35-fixed-float-helper-compaction.md](docs/runtime/phase35-fixed-float-helper-compaction.md)
- [docs/runtime/phase35-runtime-profile-variants.md](docs/runtime/phase35-runtime-profile-variants.md)
- [docs/frontend/phase36-math-header.md](docs/frontend/phase36-math-header.md)
- [docs/ir/phase36-math-lowering.md](docs/ir/phase36-math-lowering.md)
- [docs/runtime/phase36-float-math-helpers.md](docs/runtime/phase36-float-math-helpers.md)
- [docs/frontend/phase37-sqrtf.md](docs/frontend/phase37-sqrtf.md)
- [docs/ir/phase37-sqrtf-lowering.md](docs/ir/phase37-sqrtf-lowering.md)
- [docs/runtime/phase37-sqrtf-helper.md](docs/runtime/phase37-sqrtf-helper.md)
- [docs/runtime/phase38-math-profiles.md](docs/runtime/phase38-math-profiles.md)
- [docs/runtime/phase38-sqrtf-accuracy.md](docs/runtime/phase38-sqrtf-accuracy.md)
- [docs/developer-guide/math-profile-selection.md](docs/developer-guide/math-profile-selection.md)
- [docs/runtime/phase39-precise-sqrtf.md](docs/runtime/phase39-precise-sqrtf.md)
- [docs/ir/phase39-sqrtf-profile-lowering.md](docs/ir/phase39-sqrtf-profile-lowering.md)
- [docs/backend/phase39-sqrtf-resource-cost.md](docs/backend/phase39-sqrtf-resource-cost.md)
- [docs/runtime/phase40-numeric-accuracy-harness.md](docs/runtime/phase40-numeric-accuracy-harness.md)
- [docs/runtime/phase40-sqrtf-accuracy-policy.md](docs/runtime/phase40-sqrtf-accuracy-policy.md)
- [docs/frontend/phase41-fminf-fmaxf.md](docs/frontend/phase41-fminf-fmaxf.md)
- [docs/ir/phase41-minmax-lowering.md](docs/ir/phase41-minmax-lowering.md)
- [docs/runtime/phase41-minmax-helpers.md](docs/runtime/phase41-minmax-helpers.md)
- [docs/runtime/phase42-float-compare-compaction.md](docs/runtime/phase42-float-compare-compaction.md)
- [docs/runtime/phase42-minmax-compaction.md](docs/runtime/phase42-minmax-compaction.md)
- [docs/frontend/phase43-sinf-cosf.md](docs/frontend/phase43-sinf-cosf.md)
- [docs/ir/phase43-trig-lowering.md](docs/ir/phase43-trig-lowering.md)
- [docs/runtime/phase43-trig-helpers.md](docs/runtime/phase43-trig-helpers.md)
- [docs/runtime/phase43-trig-accuracy-policy.md](docs/runtime/phase43-trig-accuracy-policy.md)
- [docs/backend/phase43-trig-rom-tables.md](docs/backend/phase43-trig-rom-tables.md)
- [docs/developer-guide/math-subset.md](docs/developer-guide/math-subset.md)
- [docs/migration/phase3-to-phase4-abi.md](docs/migration/phase3-to-phase4-abi.md)
- [docs/developer-guide/adding-device.md](docs/developer-guide/adding-device.md)

## License

- compiler source, tests, docs, examples, and scripts: `GPL-3.0-or-later`
- public headers and runtime material intended for compiled firmware: `GPL-3.0-or-later WITH GCC-exception-3.1`
- [COPYING](COPYING) contains the full GNU GPLv3 text
- [COPYING.RUNTIME](COPYING.RUNTIME) contains the GCC Runtime Library Exception 3.1 text

## Current Limits

Phase 43 adds finite table-driven `sinf` / `cosf`, but current hard limits remain: no `double`, no full math library (`tanf`, `atanf`, `powf`, `expf`, `logf`, etc.), no correctly-rounded IEEE dynamic `sqrtf`, no full IEEE NaN/Inf/subnormal compliance, no ISO/IEEE NaN or signed-zero `fminf` / `fmaxf` semantics, no general-purpose libm-grade argument reduction for trig, no dynamic mixed float/integer arithmetic, no general ROM pointer model, no code-space pointers, no address-of ROM elements, no ROM/data pointer mixing, no jump tables, no case/default labels buried under other control statements, no anonymous nested aggregate fields, no signed bitfields, no multidimensional ROM arrays, no incomplete-struct/union pointers, no pointer-to-function-pointer object model, no helper-backed math inside ISR except inline `fabsf`, no fixed-point modulo, no raw computed PIC16 indirect calls, and no recursion. Float/math helpers are large; resource fitting can reject helper-heavy programs on real targets. Helper-backed float expressions are most robust when ROM-read values are first copied into RAM globals/statics before arithmetic, comparison, or math calls. Use fixed-point or validated calibration tables when exact square-root or trigonometric behavior matters outside the documented validated points and tolerance range. The emulator is intentionally core-only: no full peripheral timing model, no asynchronous interrupt scheduling, and no claim of complete PIC16 device emulation.
