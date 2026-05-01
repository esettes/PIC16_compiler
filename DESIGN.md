# `pic16cc` Design

CLI binary name for end users: `picc`.

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

## Current Technical Decisions

### Shared Backend + Descriptors

Backend knows `midrange14` family, not concrete devices. Device descriptors own RAM ranges, program size, SFRs, vectors, config words.

### Phase 4 Stack-first ABI

Current ABI is stack-first, caller-pushed, upward-growing.

Rules:

- caller pushes argument bytes left-to-right
- caller cleans argument bytes after return
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
- stack depth is computed statically over the non-recursive call graph

Recursion stays unsupported because stack depth is not checked dynamically.

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
- `const` scalar, one-dimensional array, and complete named struct objects
- file-scope `const __rom char[]` and `const __rom unsigned char[]`
- file-scope `typedef` aliases for supported object/value types
- `enum` declarations and enumerator constants
- named packed `struct` declarations with nested struct and one-dimensional array fields
- fixed-size one-dimensional arrays of supported scalar types and complete named struct types
- omitted array size when inferred from a brace initializer list or string literal
- complete named struct objects with scalar, one-dimensional array, nested struct, or supported pointer fields
- nested data-space pointers to supported scalar, pointer, or complete named struct types
- `if/else`, `while`, `for`, `do while`, `switch/case/default`, `break`, `continue`, `return`
- direct calls
- `&obj`, `*ptr`, `a[i]`, `p[i]`
- `.` and `->`
- `==`, `!=`, `<`, `<=`, `>`, `>=`
- `+`, `-`, `*`, `/`, `%`, `<<`, `>>`, `&`, `|`, `^`, `!`, `~`
- compile-time `sizeof`
- positional and designated array/struct initializer lists with zero-fill
- nested aggregate initializer lists
- string literal initialization for char/unsigned-char array fields inside structs
- whole-struct assignment between compatible complete struct types
- string literals for char/unsigned-char array initialization
- RAM-backed string literal initialization of `char *` and `const char *`
- `__rom_read8(table, index)` for explicit program-memory byte arrays
- explicit casts for supported scalar and data-pointer forms

Deferred:
- chained designators
- incomplete-struct pointers

Not implemented:

- `union`
- source-level function pointers
- multidimensional arrays
- anonymous nested struct/enum fields without declarators
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
- structs are packed in declaration order and may nest complete struct fields and one-dimensional array fields
- local aggregate initializers lower to per-slot stores; global and static initializers require constant elements and pre-materialize into byte arrays
- string literals are parsed as null-terminated byte strings and may lower to synthetic RAM-backed static array objects
- omitted array size is inferred from supported brace or string initializers before storage layout is fixed
- const objects are RAM-backed and read-only only at semantic level
- explicit `__rom` objects are file-scope-only byte arrays that bypass RAM startup data and emit as RETLW-backed program-memory tables
- designated initializers support `.field` and `[index]` forms; chained designators remain deferred
- whole-struct assignment lowers to byte-wise copies and stays rejected inside interrupt handlers
- explicit casts cover scalar conversions, data-pointer bitcasts, `(T*)0`, and pointer-to-16-bit-integer casts
- pointer comparisons use raw 16-bit RAM address ordering for compatible pointer types
- pointer subtraction lowers through inline 16-bit subtraction and optional divide-by-two scaling for compatible 1-byte/2-byte element types
- `__rom_read8()` is the only supported Phase 13 ROM read surface; ROM arrays do not decay to data pointers and ROM pointers are still unsupported
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

Phase 13 adds one dedicated ROM-read IR instruction plus backend-only ROM-table emission:

- explicit `const __rom` byte arrays become program-memory RETLW tables
- `__rom_read8(table, index)` lowers to one typed IR ROM-read instruction
- no general ROM pointer values or ROM address arithmetic are introduced

Phase 11 keeps aggregate support within the same IR model. It adds:

- recursive flattening of nested array/struct initializers into scalar slots
- designated initializer overlay before IR generation
- byte-wise whole-struct copy lowering through existing indirect load/store instructions
- no dedicated aggregate-copy or switch-table backend shortcut

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

## Phase 11 Aggregates

Phase 11 extends aggregate support without changing the packed-layout or RAM-backed data model.

Rules:

- arrays may appear inside structs
- struct fields may be other complete named structs
- nested initializer lists zero-fill omitted leaves
- designated initializers support `.field = value` and `[index] = value`
- string literals may initialize `char` / `unsigned char` array fields
- whole-struct assignment is allowed only between compatible complete struct types
- whole-struct assignment lowers as a byte-wise copy, not as a hidden helper call

Current limitations:

- multidimensional arrays remain unsupported
- chained designators such as `.outer.inner = 1` remain unsupported
- anonymous nested fields without declarators remain unsupported
- pointers to incomplete struct types remain unsupported
- local aggregate initializers and whole-struct copies remain rejected inside interrupt handlers

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

## Phase 13 ROM Objects

Phase 13 introduces one explicit program-memory object model without changing the RAM-pointer ABI.

Rules:

- syntax is `const __rom unsigned char table[] = {...};` or `const __rom char msg[] = "OK";`
- supported ROM objects are file-scope byte arrays only
- plain `const` still means RAM-backed const unless `__rom` is spelled explicitly
- ROM arrays do not decay to data-space pointers
- direct `rom_array[index]` syntax is rejected in this phase
- ROM reads use `__rom_read8(table, index)` only
- backend emits each ROM object as one callable RETLW table: entry `addwf PCL,f`, then one `retlw k` per byte
- map/listing output shows ROM symbols separately from RAM data and ordinary code

Current limitations:

- no ROM pointer types
- no local ROM objects
- no non-const ROM objects
- no ROM structs or wider-than-byte ROM element types
- no ROM reads inside interrupt handlers
