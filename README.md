# pic16cc

`pic16cc` is an experimental Rust compiler for classic 14-bit PIC16 mid-range MCUs. The pipeline is native end to end:

Installed CLI executable name: `picc`.

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

Current implementation is **Phase 7: code generation quality and optimization on top of the Phase 6 interrupt model, Phase 5 arithmetic helpers, and the Phase 4 Stack-first ABI**.

Phase 7 scope:

- no new language features
- no C-subset expansion
- better IR constant propagation, branch simplification, and dead code cleanup
- backend peephole cleanup for redundant PIC16 instruction sequences
- cheaper helper lowering for power-of-two divide/modulo and constant shifts
- cleaner banking/page handling and clearer `.map` grouping

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
- [docs/backend/phase5-helper-calling.md](/home/settes/cursus/PIC16_compiler/docs/backend/phase5-helper-calling.md:1)
- [docs/backend/phase6-interrupts.md](/home/settes/cursus/PIC16_compiler/docs/backend/phase6-interrupts.md:1)
- [docs/ir/phase4-call-lowering.md](/home/settes/cursus/PIC16_compiler/docs/ir/phase4-call-lowering.md:1)
- [docs/ir/phase5-arithmetic-lowering.md](/home/settes/cursus/PIC16_compiler/docs/ir/phase5-arithmetic-lowering.md:1)
- [docs/ir/phase6-isr-lowering.md](/home/settes/cursus/PIC16_compiler/docs/ir/phase6-isr-lowering.md:1)
- [docs/frontend/phase6-isr-syntax.md](/home/settes/cursus/PIC16_compiler/docs/frontend/phase6-isr-syntax.md:1)
- [docs/runtime/phase5-arithmetic-helpers.md](/home/settes/cursus/PIC16_compiler/docs/runtime/phase5-arithmetic-helpers.md:1)
- [docs/migration/phase3-to-phase4-abi.md](/home/settes/cursus/PIC16_compiler/docs/migration/phase3-to-phase4-abi.md:1)

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
- `+`, `-`, `*`, `/`, `%`, `<<`, `>>`, `&`, `|`, `^`
- `==`, `!=`, `<`, `<=`, `>`, `>=`
- compile-time `sizeof` for supported scalars, pointers, and fixed arrays
- indirect data access through `FSR/INDF`
- 3+ argument stack calls
- stack-backed local arrays
- pointer arguments and pointer returns
- one `void __interrupt isr(void)` handler
- interrupt vector emission at `0x0004`
- `retfie`

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
- recursion

Current constraints:

- recursion is rejected because Phase 4 computes maximum software-stack depth statically and has no runtime overflow checks
- returning a pointer to stack-local storage is rejected, including direct forms and obvious local alias chains
- explicit source casts are still limited; widening/truncation casts are primarily inserted by semantic analysis
- pointers are data-space-only; code pointers remain unsupported
- only one ISR is supported in this phase
- ISR code cannot call normal functions or Phase 5 runtime helpers
- no emulator or hardware execution runs in CI; validation is compile/listing/map/HEX shape based

## Known Limitations (Phase 6 Freeze)

- recursion is unsupported
- no runtime software-stack overflow detection is implemented
- ISR restrictions remain conservative: one ISR, no normal calls, no runtime-helper-requiring expressions
- C type-system support is partial (no `struct`, `union`, `enum`, `float`)
- source-level function pointers are unsupported
- dynamic division/modulo by zero returns `0` (constant zero divisors are diagnostics)
- pointers are data-space only; code pointers are unsupported
- pointer-to-pointer types and multidimensional arrays are unsupported

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

## Installing picc

```bash
cargo build --release
./target/release/picc --version
./target/release/picc --help

cargo install --path .
picc --version
picc --help

# Verify PATH precedence to avoid stale binary confusion
which -a picc
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

- [examples/pic16f628a/blink.c](/home/settes/cursus/PIC16_compiler/examples/pic16f628a/blink.c:1)
- [examples/pic16f628a/arith16.c](/home/settes/cursus/PIC16_compiler/examples/pic16f628a/arith16.c:1)
- [examples/pic16f628a/array_fill.c](/home/settes/cursus/PIC16_compiler/examples/pic16f628a/array_fill.c:1)
- [examples/pic16f628a/stack_abi.c](/home/settes/cursus/PIC16_compiler/examples/pic16f628a/stack_abi.c:1)
- [examples/pic16f877a/blink.c](/home/settes/cursus/PIC16_compiler/examples/pic16f877a/blink.c:1)
- [examples/pic16f877a/call_chain.c](/home/settes/cursus/PIC16_compiler/examples/pic16f877a/call_chain.c:1)
- [examples/pic16f877a/compare16.c](/home/settes/cursus/PIC16_compiler/examples/pic16f877a/compare16.c:1)
- [examples/pic16f877a/div16.c](/home/settes/cursus/PIC16_compiler/examples/pic16f877a/div16.c:1)
- [examples/pic16f877a/expression_test.c](/home/settes/cursus/PIC16_compiler/examples/pic16f877a/expression_test.c:1)
- [examples/pic16f877a/mod16.c](/home/settes/cursus/PIC16_compiler/examples/pic16f877a/mod16.c:1)
- [examples/pic16f877a/mul16.c](/home/settes/cursus/PIC16_compiler/examples/pic16f877a/mul16.c:1)
- [examples/pic16f877a/pointer16.c](/home/settes/cursus/PIC16_compiler/examples/pic16f877a/pointer16.c:1)
- [examples/pic16f877a/shift_mix.c](/home/settes/cursus/PIC16_compiler/examples/pic16f877a/shift_mix.c:1)
- [examples/pic16f628a/timer_interrupt.c](/home/settes/cursus/PIC16_compiler/examples/pic16f628a/timer_interrupt.c:1)
- [examples/pic16f877a/timer_interrupt.c](/home/settes/cursus/PIC16_compiler/examples/pic16f877a/timer_interrupt.c:1)
- [examples/pic16f877a/gpio_interrupt.c](/home/settes/cursus/PIC16_compiler/examples/pic16f877a/gpio_interrupt.c:1)

## Documentation

- [DESIGN.md](/home/settes/cursus/PIC16_compiler/DESIGN.md:1)
- [CONTRIBUTING.md](/home/settes/cursus/PIC16_compiler/CONTRIBUTING.md:1)
- [docs/architecture/overview.md](/home/settes/cursus/PIC16_compiler/docs/architecture/overview.md:1)
- [docs/backend/overview.md](/home/settes/cursus/PIC16_compiler/docs/backend/overview.md:1)
- [docs/backend/optimization.md](/home/settes/cursus/PIC16_compiler/docs/backend/optimization.md:1)
- [docs/backend/phase4-stack-first-abi.md](/home/settes/cursus/PIC16_compiler/docs/backend/phase4-stack-first-abi.md:1)
- [docs/backend/phase4-stack-model.md](/home/settes/cursus/PIC16_compiler/docs/backend/phase4-stack-model.md:1)
- [docs/ir/phase4-call-lowering.md](/home/settes/cursus/PIC16_compiler/docs/ir/phase4-call-lowering.md:1)
- [docs/backend/phase5-helper-calling.md](/home/settes/cursus/PIC16_compiler/docs/backend/phase5-helper-calling.md:1)
- [docs/ir/phase5-arithmetic-lowering.md](/home/settes/cursus/PIC16_compiler/docs/ir/phase5-arithmetic-lowering.md:1)
- [docs/runtime/phase5-arithmetic-helpers.md](/home/settes/cursus/PIC16_compiler/docs/runtime/phase5-arithmetic-helpers.md:1)
- [docs/migration/phase3-to-phase4-abi.md](/home/settes/cursus/PIC16_compiler/docs/migration/phase3-to-phase4-abi.md:1)
- [docs/developer-guide/adding-device.md](/home/settes/cursus/PIC16_compiler/docs/developer-guide/adding-device.md:1)

## Current Limits

Phase 7 does not add new language features. It only improves code quality within the existing Phase 6 language/runtime surface.
