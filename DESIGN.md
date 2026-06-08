<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# `pic16cc` Design

CLI binary names for end users:

- `picc`: compiler
- `pic16-sim`: optional simulator/debugging tool

## Goal

Build a real compiler foundation for classic PIC16 devices with strict separation between:

- frontend
- typed IR
- shared PIC16 `midrange14` backend
- per-device descriptors

## Pipeline

1. CLI
2. source manager
3. preprocessor
4. lexer
5. parser
6. semantic analysis
7. IR lowering
8. IR optimization
9. backend PIC16 `midrange14`
10. assembler/encoder
11. Intel HEX writer

## Layered Architecture

Frontend:

- `src/frontend/preprocessor.rs`
- `src/frontend/lexer.rs`
- `src/frontend/parser.rs`
- `src/frontend/semantic.rs`

IR:

- `src/ir/model.rs`
- `src/ir/lowering.rs`
- `src/ir/passes.rs`

Shared PIC16 backend:

- `src/backend/pic16/devices.rs`
- `src/backend/pic16/midrange14/codegen.rs`
- `src/backend/pic16/midrange14/encoder.rs`

Output:

- `src/assembler/listing.rs`
- `src/linker/map.rs`
- `src/hex/intel_hex.rs`

Execution validation:

- `src/sim/mod.rs`
- `src/sim_cli.rs`
- `tests/execution_sim.rs`
- `tests/sim_cli.rs`

## Current Technical Decisions

### Shared Backend + Descriptors

Backend knows `midrange14` family, not concrete devices. Device descriptors own RAM ranges, program size, SFRs, vectors, config words, reserved RAM ranges, default stack region, and ROM table region.

### Phase 25 Resource Fitting

Phase 25 validates final encoded output against the selected target. It reports program words, modeled data RAM, static data, reserved software stack, estimated max stack, ROM table words, included helpers, function-pointer dispatchers, and largest program-memory contributors.

CLI surfaces:

- `--size`
- `--memory-report`
- `--memory-report-file <path>`

The `.map` file starts with a compact memory summary. The `.lst` file starts with resource-summary comments. Overflow diagnostics are hard errors for known program-memory, data-RAM, config-word, ROM-table/code, and stack-region failures.

### Phase 26 Config + HEX Validation

Phase 26 adds explicit per-target config-word descriptors and user config syntax:

- `#pragma config FIELD = VALUE`
- `__config(0x....)` raw fallback

Config resolution happens after preprocessing and before lexing. Final HEX validation always runs after HEX emission and rejects target-range overflow, config overlap, invalid 14-bit words, checksum failure, and missing EOF. `--verify-hex` prints the validation report.

Programmer integration stays external. `picc` can print or run a user-provided command, but no PICkit USB protocol or vendor path is built into the compiler.

### Phase 31 Backend Page Safety

Phase 31 hardens the PIC16 paging model after a real Q16.16 helper regression exposed stale `PCLATH` on page-crossing control flow.

Rules:

- unconditional backend jumps lower as `pagesel target` + `goto target`
- helper calls and function calls use `pagesel target` + `call target`
- conditional branches use centralized page-safe skip sequences instead of local branch stubs
- the encoder validates final `goto` / `call` edges after layout is known
- validation errors include source symbol, from/target address, from/target page, and the tracked `PCLATH` page
- map/listing artifacts expose page metadata for debugging layout-sensitive bugs

This is backend correctness only. It adds no language feature and does not change the Stack-first ABI.

### Phase 32 Page-Aware Layout + Relaxation

Phase 32 keeps the Phase 31 validator and adds layout quality metadata plus safe linker relaxation.

Rules:

- code sections are inferred from final symbols: vectors/startup, functions, runtime helpers, fixed/float helpers, dispatchers, ROM RETLW tables, and stack trap
- per-page usage is reported in `--size`, `--memory-report`, and `.map`
- redundant same-page `setpage` pseudo-ops are removed before final encoding
- cross-page `setpage` is kept
- relaxation is iterative and final layout still passes the Phase 31 `goto` / `call` validator
- `.map` includes a `Code Layout` section for page debugging

Phase 32 does not reorder code yet. If a future layout cannot be made safe by relaxation, compilation still fails with the Phase 31 backend diagnostic.

### Phase 33 Runtime Helper Catalog + Cost Control

Phase 33 keeps runtime semantics unchanged and makes helper cost explicit:

- `RuntimeHelper` is the central catalog for helper label, ABI bytes, local/frame bytes, category, required-by metadata, dependency list, estimated words, page-sensitivity, and target constraints
- helper dependency graph validation runs before helper bodies are emitted
- helpers are still emitted only from marked runtime uses; type declarations and folded constants do not pull dead helper bodies
- `--runtime-profile small|balanced|fast` selects the helper-budget warning policy
- `--size` reports helper words by category: integer, division, fixed, float, conversion, shift, and dispatchers
- `--memory-report` includes runtime helper contributors and a dependency graph section
- `.map` groups runtime helpers by category while preserving page metadata
- `.lst` comments annotate helper category and required-by metadata

No C language feature is added in Phase 33.

### Phase 34 Runtime Helper Compaction

Phase 34 keeps the Phase 33 catalog and makes `--runtime-profile small` select a real compact helper family:

- unsigned and signed 32-bit division/modulo wrappers depend on shared `__rt_u32_divmod_core`
- helper variants are selected from the active runtime profile and reflected in the dependency graph
- `--memory-report` shows `variant=small` and `deps=__rt_u32_divmod_core` for compact wrappers
- helper-to-helper calls still use page-safe emission and Phase 31/32 validation
- stack reports include the extra helper-to-helper stack cost instead of hiding it
- representative `unsigned long` `/` plus `%` output is smaller under `small` than `balanced`

`balanced` keeps the standalone helper bodies. `fast` is still reserved and currently follows balanced behavior. No C language feature is added in Phase 34.

### Phase 35 Fixed/Float Helper Compaction

Phase 35 extends compact helper selection beyond integer division/modulo:

- signed Q16.16 multiply uses a small sign-normalizing wrapper around `__rt_mul_uq16_16`
- signed Q16.16 divide uses a small sign-normalizing wrapper around `__rt_div_uq16_16`
- finite `float` subtraction uses a compact `lhs + (-rhs)` wrapper around `__rt_f32_add`
- selected dependencies appear in `--memory-report` and the runtime helper graph
- helper-to-helper calls use the same page-safe call path and stack accounting as Phase 34

`balanced` keeps standalone helper bodies. `fast` still follows balanced behavior. No C language feature is added in Phase 35.

### Phase 36 Minimal Finite `math.h`

Phase 36 adds a deliberately small float-only math layer:

- `include/math.h` declares only `fabsf`, `truncf`, `floorf`, `ceilf`, and `roundf`
- calls are compiler-known and lower to finite f32 runtime helpers instead of external object files
- constant calls fold in semantic analysis when the argument is a finite float constant
- `roundf` uses half-away-from-zero behavior
- math helpers are cataloged as `math helper` and reported separately from arithmetic/conversion float helpers
- `fabsf` can lower inline in an ISR; `truncf`, `floorf`, `ceilf`, and `roundf` remain helper-backed and rejected in ISRs

This phase does not add `double`, `sqrtf`, trigonometry, errno, fenv, or full ISO C `math.h`.

### Phase 37 Finite `sqrtf`

Phase 37 extends the minimal math layer with one additional function:

- `include/math.h` declares `float sqrtf(float x)`
- constant finite `sqrtf` calls fold in semantic analysis
- dynamic calls lower to `__rt_f32_sqrt`
- the runtime helper stays small enough for PIC16F877A by returning exact tested finite cases and using a documented compact bit-level approximation for other positive values
- negative finite inputs return `0.0f` because this compiler does not model NaN/Inf
- `sqrtf` is demand-pruned, reported as a `math helper`, and rejected in ISRs unless folded before helper lowering

This phase still does not add `double`, trigonometry, errno, fenv, or full ISO C `math.h`.

### Phase 38 Math Profiles

Phase 38 separates math accuracy policy from runtime helper sharing:

- `--math-profile compact|balanced|precise` is parsed by the CLI and propagated to backend reports
- `compact` selects the Phase 37 compact finite `sqrtf` approximation
- `balanced` is the default and currently uses the same compact `sqrtf` helper body
- `precise` selects a larger dynamic helper variant when available
- constant `sqrtf` folding remains deterministic and uses the compile-time finite result, with negative constants folding to `0.0f`
- `--size`, `--memory-report`, `.map`, and `.lst` show the selected math profile and `__rt_f32_sqrt` variant

This phase still does not add `double`, trigonometry, exp/log/pow, errno, fenv, or full ISO C `math.h`.

### Phase 39 Precise-Profile `sqrtf`

Phase 39 implements the dynamic `sqrtf` path for `--math-profile precise`:

- compact and balanced still use `variant=compact_approx`
- precise uses `variant=precise_table_refined`
- the precise helper keeps the finite-only policy and returns `0.0f` for negative inputs
- the precise helper adds refined raw f32 results for validated non-perfect roots such as `2.0f`, `3.0f`, `10.0f`, and `0.5f`
- fallback positive finite inputs still use the compact approximation, so this is more accurate than compact for the validated set but not a correctly-rounded IEEE implementation
- resource reports show the larger helper cost and simulator tests verify compact-vs-precise behavior

This phase still does not add `double`, trigonometry, exp/log/pow, errno, fenv, or full ISO C `math.h`.

### Phase 40 Numeric Accuracy Harness

Phase 40 makes the finite `sqrtf` accuracy policy measurable:

- simulator tests compile C, run it with `pic16-sim`, read raw f32 result bits from RAM, and compare against host `f32::sqrt`
- precise `sqrtf` keeps `variant=precise_table_refined` and expands the validated table to cover `0.25f`, `0.5f`, `1.0f`, `2.0f`, `3.0f`, `4.0f`, `5.0f`, `8.0f`, `9.0f`, `10.0f`, `16.0f`, `25.0f`, `36.0f`, `49.0f`, and `64.0f`
- the documented Phase 40 tolerance for validated positive inputs in `[0.25, 64.0]` is `±0.03125`
- compact/balanced remain small approximation profiles; precise remains more accurate than compact for the tested non-perfect roots but still not correctly-rounded IEEE-754
- `--size`, `--memory-report`, `.map`, and `.lst` report the selected math profile and the finite accuracy policy

This phase still does not add `double`, trigonometry, exp/log/pow, errno, fenv, or full ISO C `math.h`.

### Phase 41 Finite `fminf` / `fmaxf`

Phase 41 adds two finite-only selection functions to the minimal math subset:

- `include/math.h` declares `float fminf(float a, float b)` and `float fmaxf(float a, float b)`
- constant calls fold when both arguments are finite compile-time `float` values
- Phase 42 compacts dynamic calls to direct `__rt_f32_cmp` plus local select code, so no separate `__rt_f32_min` / `__rt_f32_max` helper body is emitted
- `__rt_f32_cmp` now uses compact finite raw f32 ordering instead of converting operands through Q16.16
- `--size`, `--memory-report`, `.map`, `.lst`, and stack reports show the compare helper cost and omit pruned min/max wrappers
- helper-backed dynamic `fminf` / `fmaxf` remain rejected inside ISRs
- NaN/Inf and ISO/IEEE signed-zero min/max semantics are not modeled

This phase still does not add `double`, trigonometry, exp/log/pow, errno, fenv, or full ISO C `math.h`.

### Phase 43 Finite Table-Driven `sinf` / `cosf`

Phase 43 adds two finite-only trigonometric functions:

- `include/math.h` declares `float sinf(float x)` and `float cosf(float x)`
- inputs are interpreted as radians
- dynamic helpers use validated table points backed by internal ROM RETLW quarter-wave tables
- `compact` uses `variant=shared_core_compact` with tolerance `<= 0.10` on simulator-validated points after Phase 44
- default `balanced` uses `variant=shared_core_balanced` with tolerance `<= 0.05` on simulator-validated points after Phase 44
- dynamic `precise` `sinf` / `cosf` is deferred and emits a diagnostic instead of silently downgrading
- constant finite calls fold with host `f32::sin` / `f32::cos` and do not emit trig helpers
- reports show `__rt_f32_sin`, `__rt_f32_cos`, `__rt_f32_sincos_core`, and internal ROM table symbols such as `__rt_math_sin_qwave_table_balanced`

This phase still does not add `double`, `tanf`, `atanf`, exp/log/pow, errno, fenv, or full ISO C `math.h`.

### Phase 44 Trig Shared Core

Phase 44 keeps Phase 43 trig semantics but compacts runtime:

- `sinf` lowers to a small `__rt_f32_sin` wrapper
- `cosf` lowers to a small `__rt_f32_cos` wrapper
- both wrappers call one page-safe `__rt_f32_sincos_core`
- using only `sinf` does not emit the `cosf` wrapper, and using only `cosf` does not emit the `sinf` wrapper
- reports show wrapper variants, the shared core variant, helper dependencies, and ROM table contribution
- `compact` uses `variant=shared_core_compact`; default `balanced` uses `variant=shared_core_balanced`
- dynamic `precise` `sinf` / `cosf` remains deferred and diagnoses

This phase does not change trig accuracy, add new math functions, or claim libm/IEEE compliance.

### Phase 45 Trig Accuracy Harness and Range Hardening

Phase 45 keeps the Phase 44 shared-core runtime and expands validation:

- simulator trig harness compares raw f32 RAM results against host `f32::sin` / `f32::cos`
- compact tolerance remains `<= 0.10`; balanced tolerance remains `<= 0.05`
- validated dynamic range remains `[-2pi, +2pi]`
- validated points include `pi/6`, `pi/4`, `pi/3`, `pi/2`, `2pi/3`, `3pi/4`, `5pi/6`, `pi`, negative mirrors, and `±2pi`
- common moderate out-of-range aliases `±3pi` and `±4pi` are matched deterministically
- outside documented points, fallback remains coarse and finite
- constant finite trig still folds with host `f32` and emits no trig helper/table

This phase does not implement full argument reduction, precise dynamic trig, or new math functions.

### Phase 46 Moderate Trig Range Reduction

Phase 46 keeps the Phase 44 shared-core runtime and extends the finite trig alias set:

- compact tolerance remains `<= 0.10`; balanced tolerance remains `<= 0.05`
- validated table-point grid in `[-2pi, +2pi]` remains unchanged
- scoped moderate aliases now cover common multiples through `±8pi`
- reports describe the trig policy as table-driven finite matching with scoped moderate aliases, not general libm range reduction
- constant finite trig still folds with host `f32` and emits no trig helper/table

This phase does not implement continuous argument reduction, precise dynamic trig, or new math functions.

### Phase 4 Stack-first ABI

Current ABI is stack-first, caller-pushed, upward-growing.

Rules:

- caller pushes argument bytes left-to-right
- caller cleans argument bytes after return

### Phase 27 Basic Software Float

Phase 27 adds a conservative `float` scalar without adding `double` or a math library:

- storage is 32-bit little-endian IEEE-754 single-precision bits
- ABI width is the existing 32-bit Stack-first convention (`W`, `return_high`, `return_upper0`, `return_upper1`)
- supported literals are finite decimal forms with optional `f` suffix
- constant folding uses Rust `f32` encoding for finite constants
- runtime float add/sub helpers convert operands to an internal Q16.16 work format, operate there, and convert back to f32 bits
- common `* 2.0f` and `/ 2.0f` lower inline by exponent adjustment to avoid large helper pulls
- helper-backed float work is rejected inside ISRs
- ROM float tables are added in Phase 30; `double`, NaN/Inf semantics, subnormal completeness, and math functions remain deferred

This is intentionally finite-only software float support. It is not a claim of full IEEE-754 runtime compliance.

### Phase 28 Float Hardening

Phase 28 keeps the finite-only model and hardens the unsafe edges:

- dynamic `int`/`unsigned int` and fixed-point casts lower through `__rt_q16_16_to_f32` and `__rt_f32_to_q16_16`
- dynamic `long`/`unsigned long` float casts are rejected because the Phase 28 bridge is Q16.16, not a full 32-bit integer-to-float converter
- dynamic float comparisons are rejected; constant comparisons continue to fold deterministically
- cast helpers are listed as `float helper` in map, listing, memory report, and stack report data
- ISR policy remains conservative: float helper casts/arithmetic/comparisons are rejected

Phase 30.5 repairs the signed `int` round-trip path by routing dynamic integer/float casts through the Phase 29 32-bit helpers after sign/zero extension. Fixed-point float casts still use the Q16.16 bridge.

### Phase 29 Float Runtime Completion

Phase 29 fills the main remaining finite-float runtime gaps:

- dynamic comparisons lower through shared `__rt_f32_cmp`, returning `0xFF`, `0`, or `1`
- comparisons work in assignment and control-flow lowering (`if`, `while`, `for`)
- dynamic `long` / `unsigned long` to `float` use `__rt_i32_to_f32` and `__rt_u32_to_f32`
- dynamic `float` to `long` / `unsigned long` use `__rt_f32_to_i32` and `__rt_f32_to_u32`
- dynamic narrower integer float casts sign/zero extend to the same helpers and truncate back after conversion
- float-to-integer casts truncate toward zero; dynamic negative float to unsigned long returns zero
- helper costs remain visible to Phase 25 resource fitting

### Phase 30 ROM Float Tables

Phase 30 stores calibration-style float constants in program memory:

- supported form is file-scope `const __rom float table[] = { ... };`
- initializer elements are finite float constants or already-foldable float expressions
- payload bytes are raw little-endian f32 bits in RETLW tables
- constant and dynamic `table[index]` reads lower through the existing 32-bit ROM-read IR/backend path
- no startup RAM copy is emitted for ROM float objects
- ROM/data pointer mixing, address-of ROM elements, local ROM objects, non-const ROM objects, and assignment to ROM elements remain diagnostics
- dynamic ROM reads remain rejected inside ISRs; constant-index reads are allowed only when lowered inline
- helper-backed float arithmetic/comparison remains expensive and resource-fitted

### Phase 24 Dynamic Q16.16 Helpers

Phase 22/23/24 add explicit fixed-point scalar types:

- `__fixed8_8` / `__ufixed8_8`: 16-bit raw Q8.8 storage
- `__fixed16_16` / `__ufixed16_16`: 32-bit raw Q16.16 storage
- little-endian storage and existing Stack-first ABI widths
- raw constructors `__q8_8`, `__uq8_8`, `__q16_16`, and `__uq16_16`
- decimal fixed literals with explicit suffixes `q8_8`, `uq8_8`, `q16_16`, and `uq16_16`
- one-dimensional fixed-point `const __rom` calibration tables with direct indexing

Q8.8 add/sub/compare are raw byte-wise operations. Q8.8 multiply/divide lower through 32-bit raw intermediates and the existing Phase 21 helper paths. Q16.16 add/sub/compare/casts are supported; Q16.16 multiply/divide are folded for compile-time constants and lower to runtime helpers for dynamic operands.

Q16.16 dynamic helpers use private page-safe fixed-point algorithms without exposing public `long long`. Signed helpers normalize signs, run the unsigned core, and restore the final two's-complement sign. Dynamic division by zero returns raw zero, matching the existing integer helper policy; constant division by zero remains a diagnostic. Helper-backed fixed operations stay rejected inside ISRs.

### Phase 21 32-Bit Integers

`long` and `unsigned long` are 4-byte little-endian integer objects. The Stack-first ABI keeps caller-pushed arguments; 32-bit arguments consume four bytes. 32-bit returns use `W` for byte 0 and helper slots `return_high`, `return_upper0`, and `return_upper1` for bytes 1 through 3.

The backend lowers inline-safe 32-bit byte-wise operations directly and uses runtime helpers for multiply, divide, modulo, and dynamic shifts. ROM long objects remain deferred and are rejected clearly.
- callee saves caller `FP`
- callee sets `FP` to callee argument base
- callee allocates locals, local arrays, IR temps in frame storage
- callee epilogue restores `SP` to caller argument top, restores caller `FP`, returns
- 8-bit return in `W`
- 16-bit/pointer return in `W` + helper slot `return_high`

Frame layout:

- `FP + 0 .. arg_bytes - 1`: arguments
- `FP + arg_bytes`: saved caller `FP` low
- `FP + arg_bytes + 1`: saved caller `FP` high
- `FP + arg_bytes + 2 ..`: locals, arrays, IR temps

This keeps argument cleanup single-owner: caller only. Callee never subtracts caller argument bytes.

### Software Stack

Software stack is real backend state:

- `stack_ptr` and `frame_ptr` live in backend helper RAM slots
- startup initializes both to `stack_base`
- all frame accesses route through `FSR/INDF`
- target descriptors define `stack_base` and exclusive `stack_limit`
- optional `--stack-check` emits inline growth guards that branch to `__stack_overflow_trap`
- `--stack-report` surfaces per-function frame/helper/call-depth data plus ISR context cost
- stack depth is computed statically over the non-recursive call graph and expanded across known function-pointer dispatcher targets

Recursion stays unsupported in Phase 20 even when runtime stack checks are enabled.

### Phase 20 Simulator CLI

Phase 20 exposes the Phase 19 emulator through a separate `pic16-sim` binary.

Rules:

- normal `picc` compilation behavior is unchanged
- simulator usage is optional and lives outside the compiler pipeline
- `pic16-sim` loads user-provided Intel HEX files
- `.map` files are parsed only for code/data symbol lookup
- stop conditions are `--run-until <symbol>` and `--max-steps <n>`
- tracing is a simulator/debugging concern, not a backend output change
- no language support is added in this phase

Current CLI support:

- `--map <file>`
- `--run-until <symbol>`
- `--max-steps <n>`
- `--print-symbol <name>`
- `--print-regs`
- `--trace`
- `--trace-file <path>`
- `--target <name>`
- `--help`
- `--version`

### Phase 19 Execution Validation

Phase 19 added a small internal emulator for the subset of PIC16 instructions currently emitted by the backend.

Rules:

- emulator core remains reusable and separate from normal compilation
- tests compile real C inputs all the way to Intel HEX first
- simulator loads generated HEX rather than IR or backend internals
- tests stop at generated labels such as `__halt` with a hard instruction-step ceiling
- unsupported opcodes fail explicitly so backend emission drift is visible

Current scope:

- core CPU state needed by generated code: `W`, `STATUS`, `PCLATH`, `FSR/INDF`, PC, return stack, and banked RAM
- enough instruction decoding for current backend output
- simple SFR/RAM modeling only; no peripheral timing model
- `retfie` execution validation is supported, but full asynchronous interrupt timing remains deferred

### Phase 5 Arithmetic Runtime Helpers

Phase 5 adds real lowering for:

- `*`, `/`, `%`
- `<<`, `>>`

Rules:

- helper calls use same caller-pushed stack ABI as normal functions
- constant folds and tiny identities stay inline
- constant shifts stay inline
- dynamic shifts plus most multiply/divide/modulo paths lower through internal helper labels
- helper code is emitted only when used
- helper labels appear in map/listing output

Current helper families:

- multiply: shift-and-add loops
- divide/modulo: loop-based restoring division
- shifts: looped one-bit shifts with count clamp

Behavior:

- unsigned right shift is logical
- signed right shift is arithmetic
- constant shift count `>=` bit width is rejected
- dynamic shift counts clamp to operand bit width
- constant zero divisors are rejected
- dynamic zero divisors return `0`
- arithmetic is fixed-width and PIC16-oriented; overflow wraps/truncates

### Phase 6 Interrupt Model

Chosen syntax:

- `void __interrupt isr(void)`

Current interrupt policy is conservative Option A:

- at most one ISR per program
- ISR must be `void` and parameterless
- ISR cannot call normal functions
- ISR cannot use any expression that would lower through a Phase 5 runtime helper
- inline-safe arithmetic, compares, pointer dereference, locals, and direct SFR access remain allowed

Vector layout:

- `0x0000`: reset vector, direct `goto __reset_dispatch`
- `0x0004`: interrupt vector, direct `goto __interrupt_dispatch` when ISR exists
- `0x0004`: `retfie` when no ISR exists
- dispatch stubs after `0x0004` handle `PCLATH` page selection before branching to `main` startup or ISR body

ISR save/restore policy:

- save `W`, `STATUS`, `PCLATH`, `FSR`
- save backend ABI state: `return_high`, `scratch0`, `scratch1`, `stack_ptr`, `frame_ptr`
- save context into shared GPR addresses so `W` can be restored with `swapf` after `STATUS` is restored
- reuse normal stack-frame prologue/epilogue inside ISR after context is saved
- end ISR with `retfie`, never normal `return`

Stack-first ABI interaction:

- interrupted software-stack state is preserved before ISR frame allocation
- ISR may still use stack-backed locals and IR temps
- interrupted `stack_ptr` / `frame_ptr` are restored before `retfie`
- Phase 6 stack sizing adds one ISR frame on top of worst-case normal call depth

### Banking and Paging

Backend explicitly models:

- `STATUS.RP0/RP1` for direct banking
- `STATUS.IRP` for indirect bank selection
- `PCLATH<4:3>` before `CALL` / `GOTO`

## Supported C Subset

Supported:

- `void`, `char`, `unsigned char`, `int`, `unsigned int`
- functions
- globals, locals, static locals
- `const` scalar, fixed-size array, and complete named struct/union objects
- file-scope `const __rom char[]`, `const __rom unsigned char[]`, `const __rom int[]`, and `const __rom unsigned int[]`
- file-scope `typedef` aliases for supported object/value types
- `enum` declarations and enumerator constants
- named packed `struct` declarations with nested struct, named union, fixed-size array, multidimensional array, function-pointer, and basic unsigned bitfield fields
- named packed `union` declarations with supported scalar, pointer, array, struct, or union fields
- fixed-size arrays with complete explicit dimensions over supported scalar types and complete named struct/union types
- omitted array size when inferred from a brace initializer list or string literal
- complete named struct objects with scalar, fixed-size array, multidimensional array, nested struct, named union, bitfield, supported pointer, or supported function-pointer fields
- complete named union objects with supported scalar, pointer, array, struct, or union fields
- nested data-space pointers to supported scalar, pointer, or complete named struct/union types
- controlled source-level function pointers with supported scalar signatures
- `if/else`, `while`, `for`, `do while`, `switch/case/default`, `break`, `continue`, `return`
- direct calls and controlled indirect calls through compatible function-pointer values
- `&obj`, `*ptr`, `a[i]`, `p[i]`
- repeated multidimensional indexing like `matrix[i][j]`
- `.` and `->`
- basic unsigned bitfield member access and assignment
- `==`, `!=`, `<`, `<=`, `>`, `>=`
- `+`, `-`, `*`, `/`, `%`, `<<`, `>>`, `&`, `|`, `^`, `!`, `~`
- compile-time `sizeof`
- positional and designated array/struct/union initializer lists with zero-fill
- chained designated initializer paths such as `.a.x`, `[1][2]`, and `.field[1][2]`
- nested aggregate initializer lists
- string literal initialization for char/unsigned-char array fields inside structs
- whole-struct and whole-union assignment between compatible complete types
- string literals for char/unsigned-char array initialization
- RAM-backed string literal initialization of `char *` and `const char *`
- `__rom_read8(table, index)` and `__rom_read16(table, index)` for explicit program-memory arrays
- function-pointer arguments and returns as ordinary 16-bit scalar values
- explicit casts for supported scalar and data-pointer forms

Deferred:
- anonymous nested struct/union/enum fields without declarators
- incomplete-struct/union pointers
- ROM/function-pointer mixing beyond the explicit dispatcher model

Not implemented:

- signed bitfields
- pointer-to-function-pointer object models
- function-pointer arithmetic or relational comparisons
- function-pointer calls inside ISR
- ROM tables of function pointers or ROM function addresses
- floats
- recursion
- program-memory / code-space pointer models

## Invariants

- frontend never knows PIC16 encoding details
- backend never inspects AST directly
- every SFR address comes from descriptor data
- IR records operand types for compares
- 16-bit values use little-endian byte order
- pointers are 16-bit PIC16 data-space addresses
- accepted pointer and stack paths must reach real PIC16 lowering, not fake frontend-only models

## Lowering Notes

### Frontend / Semantic

- semantic analysis inserts widening/truncation casts
- narrowing diagnostics are range-aware for integer constant expressions
- representable constants may narrow without truncation diagnostics, including volatile byte SFR writes
- non-constant narrowing conversions still diagnose potential truncation
- equal-width mixed signedness compares are rejected
- equal-width mixed signedness arithmetic is rejected unless user adds an explicit cast
- expressions preserve lvalue/rvalue distinction
- array decay is explicit in typed tree
- typedef aliases are accepted at file scope only
- enum constants are global compile-time 16-bit `int` values
- structs are packed in declaration order and may nest complete struct/union fields, one-dimensional array fields, and basic unsigned bitfields
- unions place every field at offset `0` and use the maximum field size as storage size
- local aggregate initializers lower to per-slot stores; global and static initializers require constant elements and pre-materialize into byte arrays
- string literals are parsed as null-terminated byte strings and may lower to synthetic RAM-backed static array objects
- omitted array size is inferred from supported brace or string initializers before storage layout is fixed
- const objects are RAM-backed and read-only only at semantic level
- explicit `__rom` objects are file-scope-only 8-bit/16-bit integer arrays that bypass RAM startup data and emit as RETLW-backed program-memory tables
- multidimensional arrays use packed row-major layout and require every dimension beyond an optional outer inferred bound to be explicit
- designated initializers support chained `.field` / `[index]` paths over complete array/struct/union subobjects
- whole-struct and whole-union assignment lower to byte-wise copies and stay rejected inside interrupt handlers
- bitfield reads lower to load + shift + mask; writes lower to read-modify-write over the containing storage unit
- explicit casts cover scalar conversions, data-pointer bitcasts, `(T*)0`, and pointer-to-16-bit-integer casts
- pointer comparisons use raw 16-bit RAM address ordering for compatible pointer types
- pointer subtraction lowers through inline 16-bit subtraction and optional divide-by-two scaling for compatible 1-byte/2-byte element types
- `__rom_read8()` / `__rom_read16()` plus direct `rom_table[index]` reads are the supported ROM read surfaces; ROM arrays do not decay to data pointers and ROM pointers are still unsupported
- supported source-level function pointers lower as 16-bit dispatch IDs, not raw code addresses
- function-pointer calls use per-signature generated compare-chain dispatchers and stay rejected inside interrupt handlers
- switch expressions must be integer-valued; case labels must be constant and representable in the switch type
- switch lowering evaluates the controlling expression once, compares through a linear branch chain, allows fallthrough, and routes `break` to the innermost switch end
- case/default labels nested under other control statements in the same switch are rejected in phase 9
- stack-local pointer returns are rejected directly and through obvious local alias chains
- shift result type is left operand type; shift count is coerced to left operand type

Integer promotion subset:

- same integer type stays unchanged
- integer literal adopts other operand type when possible
- otherwise wider width wins
- equal-width mixed signedness is rejected unless explicit cast is present

### IR

IR models:

- `IrInstr::Cast`
- `IrInstr::AddrOf`
- `IrInstr::LoadIndirect`
- `IrInstr::StoreIndirect`
- `IrInstr::Call`
- `IrInstr::IndirectCall`
- typed branch conditions

Boolean expressions lower through branch form first. Memory expressions lower through explicit address and indirect ops.

Phase 9 does not add a dedicated switch IR terminator. The IR lowerer expands each valid switch into:

- one controlling-value evaluation
- a linear compare-and-branch dispatch chain
- ordinary CFG blocks for case/default entry points
- ordinary jumps for `break` and fallthrough

Phase 10 keeps IR free of dedicated string/static-data opcodes. Instead it uses:

- byte payloads for global and static array/struct initializers
- scalar constant expressions for scalar global/static initializers
- ordinary startup stores/clears for initialized or zero-filled RAM-backed static data
- ordinary per-slot local stores for automatic aggregate initialization

Phase 13/14 add one dedicated ROM-read IR instruction plus backend-only ROM-table emission:

- explicit `const __rom` byte arrays and 16-bit integer arrays become program-memory RETLW tables
- `__rom_read8(table, index)`, `__rom_read16(table, index)`, and direct ROM indexing lower to typed IR ROM-read instructions
- no general ROM pointer values or ROM address arithmetic are introduced

Phase 11 keeps aggregate support within the same IR model. It adds:

- recursive flattening of nested array/struct initializers into scalar slots
- designated initializer overlay before IR generation
- byte-wise whole-struct copy lowering through existing indirect load/store instructions
- no dedicated aggregate-copy or switch-table backend shortcut

Phase 15 keeps unions and bitfields inside the same aggregate model. It adds:

- packed union metadata with field offset `0` and max-field-size storage
- explicit typed bitfield lvalues that lower to ordinary shift/mask/read-modify-write IR
- byte-wise whole-union copy through the same indirect load/store path already used for structs

Phase 16 keeps multidimensional arrays inside the same explicit aggregate and address model. It adds:

- row-major fixed-size multidimensional array layout
- recursive flattening of multidimensional initializer lists into row-major scalar slots
- chained `.field` / `[index]` designator resolution before IR generation
- typed element-address computation for repeated indexing like `matrix[i][j]`

Phase 17 keeps indirect calls inside the typed IR instead of introducing raw PIC16 code pointers. It adds:

- `IrInstr::IndirectCall` carrying the callee operand, normalized function-pointer signature, and ordinary argument operands
- one dispatcher group per supported function-pointer signature
- direct function names and `&function` lowered to stable per-signature dispatch IDs
- indirect-call stack-depth accounting by expanding each signature group across its reachable concrete callees

Interrupt functions stay structurally ordinary IR functions, but carry interrupt metadata so the backend can:

- emit the interrupt vector and dispatch stub
- pick ISR prologue/epilogue instead of normal return lowering
- account for ISR frame depth separately from the normal call graph

### Backend

Backend lowers:

- 8-bit and 16-bit copy/store/load
- 8-bit and 16-bit add/sub
- 8-bit and 16-bit multiply/divide/modulo through runtime helpers
- constant and dynamic shifts
- byte-wise bitwise ops
- signed and unsigned compares
- address materialization for globals, params, locals, SFRs
- indirect scalar access through `FSR/INDF`
- stack frame reads/writes through `frame_ptr + offset`

Per-call IR temps now live in frame storage, not static absolute RAM.

## Phase 7 Optimization Layer

Phase 7 does not change the language subset or the frontend contract. It adds optimization work in two places:

- IR optimization passes before backend lowering
- backend cleanup and helper-fast-path decisions after lowering

Current pass order for `-O1`, `-O2`, and `-Os`:

1. constant propagation and folding
2. dead code elimination
3. temp-slot compaction
4. backend helper avoidance and bank/page reuse
5. backend peephole cleanup

Optimization invariants:

- Stack-first ABI is unchanged
- ISR lowering is unchanged
- runtime helpers still obey the normal call ABI
- optimizations must preserve bank/page correctness
- correctness wins over code size

Current backend quality work includes:

- constant branch simplification before codegen
- removal of unreachable IR blocks
- temp-id compaction to reduce frame pressure
- power-of-two unsigned divide lowered as inline right shift
- power-of-two unsigned modulo lowered as inline mask
- selective RP0/RP1 updates instead of blind bank rewrites
- peephole cleanup for redundant self-moves, duplicate writes, duplicate bit operations, duplicate `setpage`, and overwritten W loads

## Phase 9 Switch Lowering

Phase 9 adds switch control flow in frontend + IR lowering, not as a backend AST shortcut.

Rules:

- switch expression types: `char`, `unsigned char`, `int`, `unsigned int`, and enum-backed 16-bit `int`
- case labels use integer constant expressions or enum constants
- duplicate normalized case values are rejected
- at most one `default` label is allowed
- `break` exits only the innermost enclosing loop-or-switch construct; a `break` in a nested switch does not exit an outer loop
- fallthrough is explicit CFG flow into the next case/default label when no `break`, `return`, or other terminator intervenes

Lowering strategy:

- evaluate controlling expression once
- emit a linear equality-compare chain in IR
- branch to case/default blocks
- if no case matches and no default exists, jump to switch end
- reuse existing backend compare-branch emission; no jump tables in this phase

Current limitation:

- case/default labels must remain in the switch body flow or nested blocks, not under unrelated control statements like `if` or loop bodies

## Phase 10 Static Data

Phase 10 improves the static-data model without changing the PIC16 backend architecture.

Rules:

- string literals use null-terminated byte payloads in RAM-backed static data
- supported string escapes are `\n`, `\r`, `\t`, `\\`, `\"`, and `\0`
- `char` and `unsigned char` arrays may initialize from string literals
- explicit array sizes must fit the entire string including the trailing null byte
- omitted array sizes may be inferred from brace initializer element count or string length plus null
- globals, file-scope statics, and static locals are initialized by startup code in RAM
- missing array/struct initializer elements are zero-filled
- `const` scalar/array/flat-struct objects are RAM-backed and semantically read-only

Current limitation:

- const data is still RAM-backed rather than modeled in separate program memory
- duplicate string pooling is not attempted

## Historical Phase 11 Aggregates

Phase 11 extends aggregate support without changing the packed-layout or RAM-backed data model.

Rules:

- arrays may appear inside structs
- struct fields may be other complete named structs
- nested initializer lists zero-fill omitted leaves
- designated initializers support `.field = value` and `[index] = value`
- string literals may initialize `char` / `unsigned char` array fields
- whole-struct assignment is allowed only between compatible complete struct types
- whole-struct assignment lowers as a byte-wise copy, not as a hidden helper call

At the end of Phase 11, remaining limitations still included:

- multidimensional arrays
- chained designators such as `.outer.inner = 1`
- anonymous nested fields without declarators
- pointers to incomplete struct types
- local aggregate initializers and whole-struct copies inside interrupt handlers

## Phase 12 Pointers

Phase 12 extends the existing RAM-only pointer model without introducing code-space pointers or a new ABI.

Rules:

- pointer-to-pointer types are supported as ordinary 16-bit data-space pointer values
- const-qualified pointer forms support pointer-to-const, const pointer, and const pointer-to-const
- implicit `T *` to `const T *` conversion is accepted
- nested-pointer qualifier conversions remain conservative; deeper qualifier changes require exact match unless the user adds an explicit cast
- pointer relational comparisons use raw RAM address ordering for compatible data-space pointer types
- pointer subtraction supports compatible pointer types whose element size is 1 or 2 bytes
- string literals may initialize `char *` and `const char *` by creating anonymous RAM-backed static objects

Current limitations:

- no program-memory / code-space pointer model
- pointer subtraction assumes the pointers refer into the same object, matching ordinary C same-object expectations
- pointer subtraction rejects larger element sizes instead of introducing helper-based division

## Phase 13/14 ROM Objects

Phases 13 and 14 introduce one explicit program-memory object model without changing the RAM-pointer ABI.

Rules:

- syntax is `const __rom unsigned char table[] = {...};`, `const __rom char msg[] = "OK";`, `const __rom unsigned int table16[] = {...};`, or `const __rom int signed16[] = {...};`
- supported ROM objects are file-scope 8-bit or 16-bit integer arrays only
- plain `const` still means RAM-backed const unless `__rom` is spelled explicitly
- ROM arrays do not decay to data-space pointers
- direct `rom_array[index]` reads are supported for those arrays
- ROM reads use `__rom_read8(table, index)` / `__rom_read16(table, index)` or direct indexing
- backend emits each ROM object as one callable RETLW table: entry `addwf PCL,f`, then one `retlw k` per byte
- map/listing output shows ROM symbols separately from RAM data and ordinary code

Current limitations:

- no ROM pointer types
- no local ROM objects
- no non-const ROM objects
- no ROM structs, unions, or bitfield objects
- dynamic ROM reads inside interrupt handlers remain rejected

## Phase 16 Multidimensional Arrays

Phase 16 extends the existing packed aggregate model to fixed-size multidimensional RAM arrays.

Rules:

- multidimensional arrays use row-major layout
- every dimension is explicit except an optional outermost inferred bound from a brace initializer
- repeated indexing like `matrix[i][j]` lowers by composing one row-major byte offset
- multidimensional arrays may appear inside complete structs and unions
- multidimensional initializer lists flatten to row-major scalar slots with zero-fill
- chained designators support `.field`, `[index]`, and mixed complete paths such as `.field[1][2]`

Current limitations:

- multidimensional arrays do not decay to data pointers
- multidimensional array parameter types remain rejected instead of inventing a pointer-to-array ABI
- multidimensional `__rom` arrays remain rejected
- helper-requiring dynamic multidimensional indexing stays rejected inside interrupt handlers

## Phase 17 Function Pointers

Phase 17 adds one conservative function-pointer model for classic PIC16 without raw computed calls.

Rules:

- supported function-pointer values are represented as 16-bit dispatch IDs, not machine code addresses
- supported signatures are `void`, `char`, `unsigned char`, `int`, or `unsigned int` return with zero or one integer argument
- taking the address of a supported function or using the bare function name as a value yields a dispatch ID
- compatible function-pointer variables, arrays, and struct fields are supported
- calling through a function pointer lowers to one ordinary ABI call into a generated per-signature dispatcher
- dispatcher miss paths return zeroed return registers instead of jumping to unknown code

Current limitations:

- pointer-to-function-pointer object models remain rejected
- function-pointer arithmetic and relational comparisons remain rejected
- function-pointer calls remain rejected inside interrupt handlers
- ROM tables of function addresses and ROM function-pointer objects remain rejected

## Phase 18 Stack Safety

Phase 18 improves stack safety without changing the Stack-first ABI or enabling recursion.

Rules:

- software stack still grows upward from `stack_base` toward exclusive `stack_limit`
- backend map output exposes `__stack_base`, `__stack_limit`, `__stack_ptr`, and `__frame_ptr`
- `--stack-check` inserts inline growth checks before frame allocation, helper call argument growth, direct call argument growth, and indirect dispatcher-call argument growth
- overflow branches to `__stack_overflow_trap`, an infinite loop with no helper calls
- `--stack-report` prints a human-readable report; `--stack-report-file <path>` writes same detailed text to disk
- static call-graph analysis expands direct calls, helper cost, ISR roots, and supported function-pointer dispatcher target groups

Current limitations:

- recursion still rejects at semantic-analysis time
- stack reports remain conservative when a function-pointer signature group has no known target set
- function-pointer calls remain rejected inside interrupt handlers
