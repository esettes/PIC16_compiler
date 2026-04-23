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

### Phase 2 Memory Model

There is no software stack yet:

- parameters in fixed helper register pairs
- 8-bit return in `W`
- 16-bit return in `W` + helper byte
- locals and temporaries in static RAM slots

Trade-offs:

- simple, explicit ABI for real PIC16 codegen
- no recursion and no dynamic frames yet

### Banking and Paging

The backend explicitly models:

- `STATUS.RP0/RP1` for banking
- `PCLATH<4:3>` for `CALL`/`GOTO`

Phase 2 still uses a simple and safe policy before an optimal one.

## Phase 2 C Subset

Supported in code generation:

- `void`, `char`, `unsigned char`, `int`, `unsigned int`
- functions
- global/local variables
- `if/else`
- `while`
- `for`
- `do while`
- `break`, `continue`
- `return`
- direct calls
- `==`, `!=`, `<`, `<=`, `>`, `>=`
- `+`, `-`, `&`, `|`, `^`, `!`, `~`
- typed casts inserted by semantic analysis for widening and truncation
- boolean materialization as `0` / `1`

Deferred:

- `*`, `/`, `%`
- mixed signedness compares on equal-width operands without explicit normalization
- more than two parameters / arguments

Not implemented:

- `switch`
- pointers
- arrays
- structs
- floats
- user ISR support

## Invariants

- the frontend never knows PIC16 encoding details
- the backend never inspects the AST
- every SFR address comes from a descriptor
- IR records operand types for branch compares; backend does not infer signedness from opcodes alone
- 16-bit values always occupy two consecutive bytes in little-endian order

## Phase 2 Lowering

### Frontend / Semantic Layer

- integer literals infer to `int` or `unsigned int` depending on range
- semantic analysis inserts explicit casts for:
  - zero extension
  - sign extension
  - truncation
  - same-width bitcasts
- unsupported equal-width mixed signedness compares are rejected with diagnostics instead of silently picking a rule

### IR

Phase 2 extends the IR with:

- `IrInstr::Cast`
- typed `IrCondition::NonZero`
- typed `IrCondition::Compare`

Relational and logical comparison expressions lower through branch form first, then materialize `0` or `1` into a temp when a value is required. This keeps compare lowering reusable for:

- `if`
- loop headers
- `!expr`
- assignments such as `flag = lhs < rhs`

### Backend

The shared `midrange14` backend now lowers:

- 16-bit copy/store/load
- 16-bit add/sub with explicit carry/borrow propagation
- 16-bit bitwise ops per byte
- 16-bit casts between 8-bit and 16-bit integer forms
- 16-bit equality and inequality
- unsigned `< <= > >=`
- signed `< <= > >=`

Signed relation lowering strategy:

1. compare sign bytes first
2. if signs differ, decide from sign only
3. if signs match, reuse unsigned compare lowering

Unsigned relation lowering strategy:

1. subtract most-significant byte
2. branch from carry/zero
3. only inspect low byte when the high byte is equal

### ABI Details

- helper slots:
  - `arg0.lo`, `arg0.hi`
  - `arg1.lo`, `arg1.hi`
  - `return_high`
  - `scratch0`, `scratch1`
- helper slots are backend-managed and not user-visible symbols
- booleans are normalized `unsigned char`
- `main` still requires zero parameters in this phase

Trade-offs:

- clear, testable byte-wise lowering
- no hidden runtime helpers for operations not implemented yet
- no fake frontend-only `int` support; every accepted Phase 2 integer path reaches HEX emission

## Growth Plan

1. runtime helpers for multiply/divide/modulo
2. optional software stack
3. arrays/pointers
4. interrupts
5. stronger PIC16 peephole and bank/page optimization
6. new PIC16 mid-range targets
