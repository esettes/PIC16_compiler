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

### v0.1 Memory Model

There is no software stack yet:

- parameters in fixed registers
- return value in `W`
- locals and temporaries in static RAM

Trade-offs:

- simple and correct MVP for real examples
- no recursion and no full ABI yet

### Banking and Paging

The backend explicitly models:

- `STATUS.RP0/RP1` for banking
- `PCLATH<4:3>` for `CALL`/`GOTO`

v0.1 uses a simple and safe policy before an optimal one.

## v0.1 C Subset

Supported in code generation:

- `void`, `char`, `unsigned char`
- functions
- global/local variables
- `if/else`
- `while`
- `for`
- `do while`
- `break`, `continue`
- `return`
- direct calls
- `==`, `!=`
- `+`, `-`, `&`, `|`, `^`, `!`, `~`

Parsed but not lowered yet:

- `int`, `unsigned int`
- `*`, `/`, `%`
- `<`, `<=`, `>`, `>=`

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

## Growth Plan

1. 16-bit integers
2. unsigned/signed relational support
3. runtime helpers for multiply/divide
4. optional software stack
5. arrays/pointers
6. interrupts
7. new PIC16 mid-range targets
