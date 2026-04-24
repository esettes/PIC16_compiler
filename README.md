# pic16cc

`pic16cc` is an experimental Rust compiler for classic 14-bit PIC16 mid-range MCUs. The pipeline is native end to end:

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

Current implementation is **Phase 4: Stack-first ABI**.

What changed from Phase 3:

- all call arguments now use the software stack
- locals, local arrays, and IR temps are per-call frame storage
- 3+ argument calls are supported
- nested non-recursive calls use one coherent caller-pushed ABI
- active docs now describe stack-first behavior; old Phase 2/3 docs remain historical

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

- [DESIGN.md](/home/settes/cursus/PIC16_compiler/DESIGN.md:1)
- [docs/backend/overview.md](/home/settes/cursus/PIC16_compiler/docs/backend/overview.md:1)
- [docs/backend/phase4-stack-first-abi.md](/home/settes/cursus/PIC16_compiler/docs/backend/phase4-stack-first-abi.md:1)
- [docs/backend/phase4-stack-model.md](/home/settes/cursus/PIC16_compiler/docs/backend/phase4-stack-model.md:1)
- [docs/ir/phase4-call-lowering.md](/home/settes/cursus/PIC16_compiler/docs/ir/phase4-call-lowering.md:1)
- [docs/migration/phase3-to-phase4-abi.md](/home/settes/cursus/PIC16_compiler/docs/migration/phase3-to-phase4-abi.md:1)

## Supported Subset

Supported:

- `#include`
- object-like `#define`
- `#if`, `#ifdef`, `#ifndef`, `#else`, `#endif`
- functions
- globals
- auto locals
- static locals
- `if` / `else`
- `while`
- `for`
- `do while`
- `break` / `continue`
- `return`
- direct calls
- `char`, `unsigned char`, `int`, `unsigned int`, `void`
- fixed-size one-dimensional arrays of supported scalar types
- pointers to supported scalar types in PIC16 data memory
- `&obj`
- `*ptr`
- `a[i]`
- `p[i]`
- unary `!`, `~`, unary `-`
- `+`, `-`, `&`, `|`, `^`
- `==`, `!=`, `<`, `<=`, `>`, `>=`
- compile-time `sizeof` for supported scalars, pointers, and fixed arrays
- indirect data access through `FSR/INDF`
- 3+ argument stack calls
- stack-backed local arrays
- pointer arguments and pointer returns

Still unsupported:

- `switch`
- source-level function pointers
- pointer-to-pointer types
- multidimensional arrays
- array initializers
- pointer subtraction between two pointers
- pointer relational compares other than `==` / `!=`
- `struct`, `union`, `enum`
- `float`
- multiplication, division, modulo
- user ISR support
- recursion

Current constraints:

- recursion is rejected because Phase 4 computes maximum software-stack depth statically and has no runtime overflow checks
- returning a pointer to stack-local storage is rejected, including direct forms and obvious local alias chains
- explicit source casts are still limited; widening/truncation casts are primarily inserted by semantic analysis
- pointers are data-space-only; code pointers remain unsupported

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

Phase 4 stack-first example:

```bash
cargo run -- \
  --target pic16f628a \
  -I include \
  -O2 -Wall -Wextra \
  --emit-ir --emit-asm --map --list-file \
  -o build/stack-abi.hex \
  examples/pic16f628a/stack_abi.c
```

List targets:

```bash
cargo run -- --list-targets
```

## Examples

- [examples/pic16f628a/blink.c](/home/settes/cursus/PIC16_compiler/examples/pic16f628a/blink.c:1)
- [examples/pic16f628a/arith16.c](/home/settes/cursus/PIC16_compiler/examples/pic16f628a/arith16.c:1)
- [examples/pic16f628a/array_fill.c](/home/settes/cursus/PIC16_compiler/examples/pic16f628a/array_fill.c:1)
- [examples/pic16f628a/stack_abi.c](/home/settes/cursus/PIC16_compiler/examples/pic16f628a/stack_abi.c:1)
- [examples/pic16f877a/blink.c](/home/settes/cursus/PIC16_compiler/examples/pic16f877a/blink.c:1)
- [examples/pic16f877a/call_chain.c](/home/settes/cursus/PIC16_compiler/examples/pic16f877a/call_chain.c:1)
- [examples/pic16f877a/compare16.c](/home/settes/cursus/PIC16_compiler/examples/pic16f877a/compare16.c:1)
- [examples/pic16f877a/pointer16.c](/home/settes/cursus/PIC16_compiler/examples/pic16f877a/pointer16.c:1)

## Documentation

- [DESIGN.md](/home/settes/cursus/PIC16_compiler/DESIGN.md:1)
- [CONTRIBUTING.md](/home/settes/cursus/PIC16_compiler/CONTRIBUTING.md:1)
- [docs/architecture/overview.md](/home/settes/cursus/PIC16_compiler/docs/architecture/overview.md:1)
- [docs/backend/overview.md](/home/settes/cursus/PIC16_compiler/docs/backend/overview.md:1)
- [docs/backend/phase4-stack-first-abi.md](/home/settes/cursus/PIC16_compiler/docs/backend/phase4-stack-first-abi.md:1)
- [docs/backend/phase4-stack-model.md](/home/settes/cursus/PIC16_compiler/docs/backend/phase4-stack-model.md:1)
- [docs/ir/phase4-call-lowering.md](/home/settes/cursus/PIC16_compiler/docs/ir/phase4-call-lowering.md:1)
- [docs/migration/phase3-to-phase4-abi.md](/home/settes/cursus/PIC16_compiler/docs/migration/phase3-to-phase4-abi.md:1)
- [docs/developer-guide/adding-device.md](/home/settes/cursus/PIC16_compiler/docs/developer-guide/adding-device.md:1)

## Next Work

Planned after Phase 4 repair:

1. runtime helpers for multiply/divide/modulo
2. richer pointer compatibility and initializer support
3. interrupts
4. stronger PIC16 banking/page optimization
5. more PIC16 mid-range targets
