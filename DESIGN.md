# `pic16cc` Design

## Goal

Build a real compiler foundation for classic PIC16 devices, with strict separation between the frontend, IR, shared `midrange14` backend, and per-device descriptors.

## Pipeline

1. CLI
2. Source manager
3. Preprocessor
4. Lexer
5. Parser
6. Semantic analysis + symbols + typing
7. IR lowering
8. Opt passes
9. Backend PIC16 `midrange14`
10. Assembler/encoder
11. Intel HEX writer

## Layered Architecture

### Frontend

- `src/frontend/preprocessor.rs`
- `src/frontend/lexer.rs`
- `src/frontend/parser.rs`
- `src/frontend/semantic.rs`

### IR

- `src/ir/model.rs`
- `src/ir/lowering.rs`
- `src/ir/passes.rs`

### Shared PIC16 Backend

- `src/backend/pic16/devices.rs`
- `src/backend/pic16/midrange14/codegen.rs`
- `src/backend/pic16/midrange14/encoder.rs`

### Output

- `src/assembler/listing.rs`
- `src/linker/map.rs`
- `src/hex/intel_hex.rs`

## Technical Decisions

### Shared Backend + Descriptors

The backend knows the `midrange14` family, not concrete devices. RAM details, program memory, SFRs, vectors, and configuration words live in descriptors.

### Phase 3 Memory Model

There is no software stack yet:

- parameters in fixed helper register pairs
- 8-bit return in `W`
- 16-bit return in `W` + helper byte
- 16-bit pointer return in `W` + helper byte
- locals and temporaries in static RAM slots
- local arrays in contiguous static RAM slots

Trade-offs:

- simple, explicit ABI for real PIC16 codegen
- no recursion and no dynamic frames yet

### Banking and Paging

The backend explicitly models:

- `STATUS.RP0/RP1` for banking
- `PCLATH<4:3>` for `CALL`/`GOTO`

Phase 2 still uses a simple and safe policy before an optimal one.

## Phase 3 C Subset

Supported in code generation:

- `void`, `char`, `unsigned char`, `int`, `unsigned int`
- functions
- global/local variables
- fixed-size one-dimensional arrays of supported scalar types
- data pointers to supported scalar types
- `if/else`
- `while`
- `for`
- `do while`
- `break`, `continue`
- `return`
- direct calls
- `&obj`
- `*ptr`
- `a[i]`
- `p[i]`
- `==`, `!=`, `<`, `<=`, `>`, `>=`
- `+`, `-`, `&`, `|`, `^`, `!`, `~`
- typed casts inserted by semantic analysis for widening and truncation
- boolean materialization as `0` / `1`
- compile-time `sizeof` for supported scalar, pointer, and array types

Deferred:

- `*`, `/`, `%`
- mixed signedness compares on equal-width operands without explicit normalization
- more than two parameters / arguments
- richer pointer compatibility rules
- array initializers

Not implemented:

- `switch`
- pointer-to-pointer
- function pointers
- multidimensional arrays
- structs
- floats
- user ISR support

## Invariants

- the frontend never knows PIC16 encoding details
- the backend never inspects the AST
- every SFR address comes from a descriptor
- IR records operand types for branch compares; backend does not infer signedness from opcodes alone
- 16-bit values always occupy two consecutive bytes in little-endian order
- pointers are always 16-bit data-space addresses in this phase
- arrays only exist as addressable objects; value contexts lower them through explicit decay

## Phase 3 Lowering

### Frontend / Semantic Layer

- integer literals infer to `int` or `unsigned int` depending on range
- semantic analysis inserts explicit casts for:
  - zero extension
  - sign extension
  - truncation
  - same-width bitcasts
- unsupported equal-width mixed signedness compares are rejected with diagnostics instead of silently picking a rule
- expressions carry enough information to distinguish lvalues from rvalues
- array lvalues decay through an explicit typed node instead of implicit backend special-casing
- indexing lowers semantically into pointer arithmetic plus dereference
- `sizeof` folds to a compile-time `unsigned int` literal during semantic analysis

### IR

Phase 3 extends the IR with:

- `IrInstr::Cast`
- `IrInstr::AddrOf`
- `IrInstr::LoadIndirect`
- `IrInstr::StoreIndirect`
- typed `IrCondition::NonZero`
- typed `IrCondition::Compare`

Relational and logical comparison expressions lower through branch form first, then materialize `0` or `1` into a temp when a value is required. This keeps compare lowering reusable for:

- `if`
- loop headers
- `!expr`
- assignments such as `flag = lhs < rhs`

Memory-oriented expressions lower through explicit address and indirect memory operations:

1. named objects become lvalues in the typed tree
2. array decay produces a pointer-valued IR temp through `AddrOf`
3. `*ptr` in value position becomes `LoadIndirect`
4. `*ptr = value` becomes `StoreIndirect`
5. `a[i]` and `p[i]` lower to pointer add/sub with element-size scaling followed by dereference

### Backend

The shared `midrange14` backend now lowers:

- 16-bit copy/store/load
- 16-bit add/sub with explicit carry/borrow propagation
- 16-bit bitwise ops per byte
- 16-bit casts between 8-bit and 16-bit integer forms
- 16-bit equality and inequality
- unsigned `< <= > >=`
- signed `< <= > >=`
- pointer `==` and `!=`
- address materialization for globals, locals, params, and SFR symbols
- indirect scalar loads and stores through `FSR/INDF`
- 8-bit and 16-bit indexed access with byte-wise element scaling

Signed relation lowering strategy:

1. compare sign bytes first
2. if signs differ, decide from sign only
3. if signs match, reuse unsigned compare lowering

Unsigned relation lowering strategy:

1. subtract most-significant byte
2. branch from carry/zero
3. only inspect low byte when the high byte is equal

Indirect memory strategy:

1. direct named-object accesses still use banked file-register instructions
2. pointer low byte programs `FSR`
3. pointer high byte drives `STATUS.IRP`
4. `INDF` performs the final byte load/store
5. 16-bit indirect objects lower as two byte-wise accesses at `ptr` and `ptr + 1`

### ABI Details

- helper slots:
  - `arg0.lo`, `arg0.hi`
  - `arg1.lo`, `arg1.hi`
  - `return_high`
  - `scratch0`, `scratch1`
- helper slots are backend-managed and not user-visible symbols
- booleans are normalized `unsigned char`
- `main` still requires zero parameters in this phase
- pointers use the same 16-bit calling and return path as `unsigned int`
- supported pointers target PIC16 data memory only
- null is `0x0000`

Trade-offs:

- clear, testable byte-wise lowering
- no hidden runtime helpers for operations not implemented yet
- no fake frontend-only memory support; every accepted Phase 3 pointer/array path reaches HEX emission

## Growth Plan

1. runtime helpers for multiply/divide/modulo
2. optional software stack
3. richer pointer compatibility and initializer support
4. interrupts
5. stronger PIC16 peephole and bank/page optimization
6. new PIC16 mid-range targets
