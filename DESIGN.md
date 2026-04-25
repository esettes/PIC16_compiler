# `pic16cc` Design

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
- fixed-size one-dimensional arrays of supported scalar types
- data pointers to supported scalar types
- `if/else`, `while`, `for`, `do while`, `break`, `continue`, `return`
- direct calls
- `&obj`, `*ptr`, `a[i]`, `p[i]`
- `==`, `!=`, `<`, `<=`, `>`, `>=`
- `+`, `-`, `*`, `/`, `%`, `<<`, `>>`, `&`, `|`, `^`, `!`, `~`
- compile-time `sizeof`

Deferred:
- richer pointer compatibility
- array initializers

Not implemented:

- `switch`
- pointer-to-pointer
- function pointers
- multidimensional arrays
- structs
- floats
- user ISR support
- recursion

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
- equal-width mixed signedness compares are rejected
- equal-width mixed signedness arithmetic is rejected unless user adds an explicit cast
- expressions preserve lvalue/rvalue distinction
- array decay is explicit in typed tree
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

## Growth Plan

After Phase 5:

1. richer pointer compatibility and initializer support
2. interrupts
3. stronger PIC16 peephole/bank/page optimization
4. more PIC16 mid-range targets
