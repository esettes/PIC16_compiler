# pic16cc

`pic16cc` is an experimental Rust compiler for Linux targeting classic 14-bit PIC16 mid-range MCUs through a shared reusable backend.

Initial supported devices:

- `PIC16F628A`
- `PIC16F877A`

Final outputs:

- programmable Intel HEX (`.hex`)
- symbol map (`.map`)
- listing (`.lst`)
- optional token, AST, IR, and assembly dumps

The compiler does not wrap XC8, SDCC, or LLVM. The pipeline is native end to end:

1. preprocessing
2. lexing
3. parsing
4. semantic analysis
5. typed IR lowering
6. IR optimization
7. shared PIC16 `midrange14` lowering/codegen
8. 14-bit instruction encoding
9. Intel HEX emission

## Implementation Language

Chosen language: **Rust**.

Why Rust:

- strong typing across frontend, IR, backend, and device layers
- good Linux tooling with `cargo check`, `cargo test`, and `cargo clippy`
- predictable performance without a GC
- ownership helps keep cross-layer APIs explicit and maintainable
- unit and integration testing are easy to keep close to the code

## Phase 3 Status

Phase 3 extends the original v0.1 and Phase 2 pipeline without changing the architecture. The shared `midrange14` backend remains common to both devices; only device descriptors vary.

### Fully supported in Phase 3

- `#include`
- `#define` object-like macros
- `#if`, `#ifdef`, `#ifndef`, `#else`, `#endif`
- functions
- global variables
- local variables
- `if` / `else`
- `while`
- `for`
- `do while`
- `break` / `continue`
- `return`
- direct function calls
- `char`, `unsigned char`, `int`, `unsigned int`, `void`
- access to SFRs through target headers
- unary `!`, `~`, unary `-`
- arithmetic `+`, `-`
- bitwise `&`, `|`, `^`
- equality/inequality `==`, `!=`
- relational comparisons `<`, `<=`, `>`, `>=`
- 8-bit and 16-bit temporaries, locals, globals, arguments, and return values inside the current ABI limits
- fixed-size one-dimensional arrays of `char`, `unsigned char`, `int`, and `unsigned int`
- constrained data pointers to `char`, `unsigned char`, `int`, and `unsigned int`
- `&obj`
- `*ptr`
- `a[i]`
- `p[i]`
- array-to-pointer decay in value contexts
- pointer `+ integer`
- pointer `- integer`
- pointer `==` and `!=`
- pointer arguments and pointer returns inside the current two-argument ABI
- compile-time `sizeof` for supported scalars, pointers, and fixed arrays
- indirect PIC16 data-memory load/store through `FSR/INDF`
- valid Intel HEX generation for `PIC16F628A` and `PIC16F877A`

### Partially supported in Phase 3

- booleans are materialized as `unsigned char` values normalized to `0` or `1`
- parameters and call arguments are limited to **two** values
- mixed signedness comparisons on equal-width integer operands require explicit user-side normalization; implicit mixed signedness compare lowering is rejected
- `const` data still lowers into startup-initialized RAM, not dedicated ROM placement
- pointers are **data-space-only** in this phase; code pointers and function pointers are not supported
- pointer-compatible assignments and comparisons require matching pointee types or literal `0` as the null pointer constant
- array bounds in declarations must currently be positive integer literals
- locals, local arrays, and temporaries still use static RAM slots; there is no software stack yet

### Still unsupported

- `switch`
- pointer-to-pointer types
- function pointers
- multidimensional arrays
- taking the address of a whole array object
- pointer relational comparisons other than `==` / `!=`
- pointer subtraction between two pointers
- array initializers
- `struct`, `union`, `enum`
- `float`
- recursion
- user ISR support
- software stack / dynamic stack frames
- multiplication, division, modulo
- more than two function parameters/arguments in the current ABI

## Phase 3 ABI and Memory Model

- `char`: 8-bit signed
- `unsigned char`: 8-bit unsigned
- `int`: 16-bit signed
- `unsigned int`: 16-bit unsigned
- `sizeof(...)` result type: `unsigned int`
- 16-bit storage order: little-endian in RAM slots (`low byte = base`, `high byte = base + 1`)
- pointer size: 16 bits
- pointer representation: little-endian data-space address
- null pointer representation: `0x0000`
- supported pointer targets: `char`, `unsigned char`, `int`, `unsigned int`
- pointer address space: PIC16 data memory only
- locals and temporaries: static RAM slots, no software stack yet
- local arrays: contiguous static RAM slots in the current function storage region
- global arrays: contiguous static RAM slots initialized or zeroed at startup according to the current model
- argument passing:
  - argument 0 uses helper pair `arg0.lo` / `arg0.hi`
  - argument 1 uses helper pair `arg1.lo` / `arg1.hi`
- return values:
  - 8-bit return in `W`
  - 16-bit return in `W` for low byte and backend helper slot `return_high` for high byte
  - pointer return follows the same 16-bit return rule
- boolean results: normalized `0` or `1` in an `unsigned char`
- compare lowering:
  - equality compares high byte then low byte for 16-bit values
  - unsigned relations use PIC16 carry/zero flags after byte-wise subtraction
  - signed relations first inspect sign-byte mismatch, then reuse unsigned compare flow when signs match
- banking: explicit `STATUS.RP0/RP1`
- paging: explicit `PCLATH<4:3>` before `CALL` / `GOTO`
- indirect data access:
  - direct named-object access uses banked file-register instructions
  - pointer-based access loads `FSR` from the pointer low byte
  - `STATUS.IRP` is derived from the pointer high byte
  - `INDF` performs the final indirect byte load/store
  - 16-bit indirect objects are lowered byte by byte at `ptr` and `ptr + 1`

More detail: [DESIGN.md](/home/settes/cursus/PIC16_compiler/DESIGN.md:1), [docs/backend/phase2-abi.md](/home/settes/cursus/PIC16_compiler/docs/backend/phase2-abi.md:1), [docs/backend/phase3-memory-model.md](/home/settes/cursus/PIC16_compiler/docs/backend/phase3-memory-model.md:1), and [docs/ir/phase3-memory.md](/home/settes/cursus/PIC16_compiler/docs/ir/phase3-memory.md:1).

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

## Usage

```bash
cargo run -- \
  --target pic16f628a \
  -I include \
  -O2 -Wall -Wextra \
  --emit-ast --emit-ir --emit-asm \
  --map --list-file \
  -o build/blink.hex \
  examples/pic16f628a/blink.c
```

Phase 2 16-bit example:

```bash
cargo run -- \
  --target pic16f877a \
  -I include \
  -O2 -Wall -Wextra \
  --emit-ir --emit-asm --map --list-file \
  -o build/compare16.hex \
  examples/pic16f877a/compare16.c
```

List targets:

```bash
cargo run -- --list-targets
```

## Examples

- [examples/pic16f628a/blink.c](/home/settes/cursus/PIC16_compiler/examples/pic16f628a/blink.c:1)
- [examples/pic16f628a/arith16.c](/home/settes/cursus/PIC16_compiler/examples/pic16f628a/arith16.c:1)
- [examples/pic16f628a/array_fill.c](/home/settes/cursus/PIC16_compiler/examples/pic16f628a/array_fill.c:1)
- [examples/pic16f877a/blink.c](/home/settes/cursus/PIC16_compiler/examples/pic16f877a/blink.c:1)
- [examples/pic16f877a/compare16.c](/home/settes/cursus/PIC16_compiler/examples/pic16f877a/compare16.c:1)
- [examples/pic16f877a/pointer16.c](/home/settes/cursus/PIC16_compiler/examples/pic16f877a/pointer16.c:1)

Phase 3 pointer/array example:

```bash
cargo run -- \
  --target pic16f877a \
  -I include \
  -O2 -Wall -Wextra \
  --emit-ir --emit-asm --map --list-file \
  -o build/pointer16.hex \
  examples/pic16f877a/pointer16.c
```

## Documentation

- [DESIGN.md](/home/settes/cursus/PIC16_compiler/DESIGN.md:1)
- [CONTRIBUTING.md](/home/settes/cursus/PIC16_compiler/CONTRIBUTING.md:1)
- [docs/architecture/overview.md](/home/settes/cursus/PIC16_compiler/docs/architecture/overview.md:1)
- [docs/ir/phase2-lowering.md](/home/settes/cursus/PIC16_compiler/docs/ir/phase2-lowering.md:1)
- [docs/ir/phase3-memory.md](/home/settes/cursus/PIC16_compiler/docs/ir/phase3-memory.md:1)
- [docs/backend/phase2-abi.md](/home/settes/cursus/PIC16_compiler/docs/backend/phase2-abi.md:1)
- [docs/backend/phase3-memory-model.md](/home/settes/cursus/PIC16_compiler/docs/backend/phase3-memory-model.md:1)
- [docs/developer-guide/adding-device.md](/home/settes/cursus/PIC16_compiler/docs/developer-guide/adding-device.md:1)

## Remaining Phases

1. runtime helpers for multiply/divide/modulo
2. optional software stack and wider ABI
3. richer pointer compatibility and initializer support
4. ISR support
5. stronger PIC16 banking/page optimization passes
6. more PIC16 mid-range descriptors without backend duplication
