# pic16cc

`pic16cc` is an experimental Rust compiler for Linux targeting the 14-bit PIC16 mid-range family. This first iteration builds a real compiler foundation with a full pipeline and a shared reusable backend for:

- `PIC16F628A`
- `PIC16F877A`

Final outputs:

- programmable Intel HEX (`.hex`)
- symbol map (`.map`)
- listing (`.lst`)
- optional token, AST, IR, and assembly dumps

It does not use XC8 or SDCC as a backend. The pipeline is native: preprocessing, frontend, IR, PIC16 lowering, in-memory assembly, and Intel HEX emission.

## Implementation Language

Chosen language: **Rust**.

Reasons:

- strong typing and ownership: reduces architecture errors across the compiler pipeline
- good frontend/backend performance without GC
- clean modularity through modules and types
- solid Linux tooling (`cargo check`, `cargo test`, `cargo fmt`, `cargo clippy`)
- integrated and maintainable tests from day one

## v0.1 Status

v0.1 prioritizes correct long-term architecture and a real working pipeline. It does not try to support all of C yet.

Currently supported:

- `#include`
- `#define` object-like
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
- `char`, `unsigned char`, `void`
- access to SFRs through target headers
- comparisons `==` and `!=`
- expressions using `+`, `-`, `&`, `|`, `^`, `!`, `~`
- valid Intel HEX `.hex` generation for classic PIC16 mid-range devices

Explicit v0.1 limitations:

- `switch` is not implemented
- `struct`, `union`, `enum`, arrays, and pointers are not implemented
- `float` is not implemented
- multiplication, division, modulo, and `< <= > >=` comparisons are parsed in the frontend, but the backend does not lower them yet
- parameters: maximum 2, 8-bit only
- `main` with no parameters
- no recursion
- no user interrupt support yet
- automatic variables use static RAM slots, not a real stack
- `const` currently lives in startup-initialized RAM, not separate ROM storage

## v0.1 ABI and Memory Model

- `char`: 8-bit, signed
- `unsigned char`: 8-bit
- `int`: 16-bit in the frontend; v0.1 codegen does not lower it yet
- `unsigned int`: 16-bit in the frontend; v0.1 codegen does not lower it yet
- 8-bit function return: `W` register
- parameters: fixed registers `__arg0`, `__arg1`
- software stack: not present in v0.1
- PIC hardware stack: subroutine return only
- locals and temporaries: static RAM slots
- banking: selected through `STATUS.RP0/RP1`
- paging: selected through `PCLATH<4:3>` before `CALL`/`GOTO`
- reset vector: `0x0000`
- interrupt vector: `0x0004`
- config word: emitted at `0x2007`

## Build

Requirements:

- Linux
- recent stable Rust

Commands:

```bash
cargo check
cargo test
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

```bash
cargo run -- --list-targets
```

## Documentation

- [DESIGN.md](/home/settes/cursus/PIC16_compiler/DESIGN.md:1)
- [CONTRIBUTING.md](/home/settes/cursus/PIC16_compiler/CONTRIBUTING.md:1)
- [docs/architecture/overview.md](/home/settes/cursus/PIC16_compiler/docs/architecture/overview.md:1)
- [docs/developer-guide/adding-device.md](/home/settes/cursus/PIC16_compiler/docs/developer-guide/adding-device.md:1)

## Remaining Phases

1. extend lowering/backend for relational comparisons and 16-bit operations
2. introduce an optional stack model and extended ABI
3. add initial array/pointer support
4. add ISR support and interrupt prologue/epilogue generation
5. add stronger PIC16 optimizations: banking/page scheduling, peephole, coalescing
6. add new PIC16 mid-range descriptors without backend duplication
