<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

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

Current implementation is **Phase 13: explicit program-memory const/string/table support on top of Phase 12 richer data-space pointers, Phase 11 aggregate completeness, Phase 10 string/static-data cleanup, Phase 9 `switch` control flow, Phase 8 type-system work, Phase 7 optimization, Phase 6 interrupts, Phase 5 arithmetic helpers, and the Phase 4 Stack-first ABI**.

Phase 13 scope:

- explicit `__rom` program-memory byte-array objects at file scope
- `const __rom unsigned char[]` and `const __rom char[]` tables/strings
- `__rom_read8(table, index)` for ROM byte reads
- separate map/listing section for ROM symbols
- explicit rejection of ROM/data-space pointer mixing
- no general ROM pointer model yet

Phase 12 scope remains:

- pointer-to-pointer types in the current data-space pointer model
- const-qualified pointer forms: pointer-to-const, const pointer, const pointer-to-const
- pointer relational comparisons for compatible data-space pointers
- pointer subtraction for compatible data-space pointers with 1-byte or 2-byte elements
- RAM-backed string literal objects that may initialize `char *` and `const char *`
- clearer pointer diagnostics for qualifier discard, invalid comparisons, and invalid subtraction

Phase 11 scope remains:

- arrays inside structs
- nested struct fields with composed offsets for `.` and `->`
- nested aggregate initializer lists with zero-fill
- designated initializers for `.field = value` and `[index] = value`
- string literal initialization for char/unsigned-char array fields
- whole-struct assignment for compatible complete struct types
- byte-wise struct-copy lowering through ordinary indirect load/store paths

Phase 10 scope remains:

- string literal lexing/parsing with `\n`, `\r`, `\t`, `\\`, `\"`, and `\0` escapes
- `char` / `unsigned char` arrays initialized from string literals
- omitted array-size inference from brace initializers and string literals
- RAM-backed `const` scalar/array/flat-struct objects with semantic write rejection
- startup-time initialization and zero-fill for globals, file-scope statics, and static locals
- clearer startup/listing/map output for initialized or zeroed static data

Phase 9 scope remains:

- `switch` statements over `char`, `unsigned char`, `int`, `unsigned int`, and enum-backed 16-bit `int`
- `case` labels with integer constant expressions or enum constants
- `default` labels
- `break` from switches
- ordinary C fallthrough between adjacent cases
- explicit diagnostics for duplicate or invalid switch labels

Phase 8 scope remains:

- `typedef` aliases for supported scalar/pointer/array/object forms
- `enum` declarations with implicit and explicit enumerator values
- named `struct` declarations with packed field layout metadata
- field access lowering for `.` and `->`
- array and struct positional initializer lists with zero-fill
- clearer explicit cast handling for scalar/pointer subsets
- explicit diagnostics for unsupported aggregate operations

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
- Phase 8 adds typedef/enum/struct support, aggregate initializers, and explicit casts for supported forms
- Phase 9 adds compare-chain lowering for `switch` / `case` / `default`, fallthrough, and switch-aware `break`
- Phase 10 adds string literal parsing, RAM-backed const/static initialization cleanup, and clearer startup data artifacts
- Phase 11 adds nested aggregate layout/init support, designated initializers, and byte-wise whole-struct copy
- Phase 12 adds nested data pointers, const-qualified pointer forms, pointer compare/subtract, and RAM-backed string-literal pointer initialization
- Phase 13 adds explicit `__rom` byte arrays, RETLW-backed ROM tables, `__rom_read8()`, and separate ROM map/listing output
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
- [docs/ir/phase8-aggregate-lowering.md](/home/settes/cursus/PIC16_compiler/docs/ir/phase8-aggregate-lowering.md:1)
- [docs/ir/phase9-switch-lowering.md](/home/settes/cursus/PIC16_compiler/docs/ir/phase9-switch-lowering.md:1)
- [docs/ir/phase10-static-initializers.md](/home/settes/cursus/PIC16_compiler/docs/ir/phase10-static-initializers.md:1)
- [docs/ir/phase11-aggregate-initializers.md](/home/settes/cursus/PIC16_compiler/docs/ir/phase11-aggregate-initializers.md:1)
- [docs/frontend/phase6-isr-syntax.md](/home/settes/cursus/PIC16_compiler/docs/frontend/phase6-isr-syntax.md:1)
- [docs/frontend/phase8-types.md](/home/settes/cursus/PIC16_compiler/docs/frontend/phase8-types.md:1)
- [docs/frontend/phase9-switch.md](/home/settes/cursus/PIC16_compiler/docs/frontend/phase9-switch.md:1)
- [docs/frontend/phase10-string-literals.md](/home/settes/cursus/PIC16_compiler/docs/frontend/phase10-string-literals.md:1)
- [docs/frontend/phase11-aggregates.md](/home/settes/cursus/PIC16_compiler/docs/frontend/phase11-aggregates.md:1)
- [docs/backend/phase8-struct-layout.md](/home/settes/cursus/PIC16_compiler/docs/backend/phase8-struct-layout.md:1)
- [docs/backend/phase9-switch-codegen.md](/home/settes/cursus/PIC16_compiler/docs/backend/phase9-switch-codegen.md:1)
- [docs/backend/phase10-data-layout.md](/home/settes/cursus/PIC16_compiler/docs/backend/phase10-data-layout.md:1)
- [docs/backend/phase11-aggregate-copy.md](/home/settes/cursus/PIC16_compiler/docs/backend/phase11-aggregate-copy.md:1)
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

## Phase 8 Type-System Summary

`typedef`:

- file-scope typedef aliases are supported for scalar, pointer, array, and named struct object forms
- duplicate typedef names are rejected
- typedef/object-function name conflicts are rejected in this phase

`enum`:

- implicit enumerators start at `0` and increment by `1`
- explicit enumerator constants are supported
- enum constants are compile-time integer constants usable in expressions
- enum representation is fixed to 16-bit `int` in this phase

`struct`:

- named structs are supported
- layout is packed declaration order with no inserted padding
- field offsets are byte offsets from base address
- `.` and `->` lower through base-pointer + constant-offset addressing
- whole-struct copy assignment is rejected in this phase

Initializers:

- scalar expression initializers remain supported
- array and struct positional initializer lists are supported
- missing aggregate elements are zero-filled
- too many initializer elements are diagnosed
- designated initializers are currently rejected with an explicit diagnostic

Explicit casts:

- scalar widening/narrowing and signedness casts are supported
- explicit narrowing suppresses implicit narrowing warnings
- casts between supported data pointers are supported
- integer-to-pointer is restricted to integer zero (`(T*)0`)
- pointer-to-integer is restricted to 16-bit integer targets

## Phase 9 Switch Summary

Supported:

- `switch (expr)` over `char`, `unsigned char`, `int`, `unsigned int`, and enum-backed 16-bit `int`
- `case` labels using integer constant expressions or enum constants
- one optional `default` label per switch
- `break` exits the innermost enclosing switch; in mixed loop/switch nesting it does not exit an outer loop
- fallthrough between adjacent cases when no `break`, `return`, or other control transfer intervenes
- nested switches
- switches inside loops and loops inside switches
- compare-and-branch lowering; no jump tables in this phase

Diagnostics:

- duplicate case values
- multiple `default` labels
- `case` / `default` outside switches
- non-constant case labels
- case values not representable in the switch expression type
- unsupported non-integer switch expressions
- case/default labels nested under other control statements in the same switch are rejected in this phase

## Phase 10 Static-Data Summary

Supported:

- string literals parse as null-terminated byte strings
- supported escapes: `\n`, `\r`, `\t`, `\\`, `\"`, `\0`
- `char` and `unsigned char` arrays may initialize from string literals
- omitted array size may be inferred from brace initializer element count or string length plus trailing null
- globals, file-scope statics, and static locals are zero-initialized when no initializer is present
- scalar, array, and flat-struct globals/statics initialize through startup code
- missing array/struct initializer elements are zero-filled
- `const` scalar, array, and flat-struct objects are read-only at semantic level
- map entries annotate const/static data; startup/listing output annotates zero/init actions

Diagnostics:

- unterminated string literals
- unsupported string escape sequences
- string initializers that do not fit including the trailing null byte
- string literals used with incompatible scalar or pointer targets
- non-constant global/static initializer forms
- assignment to const objects

## Phase 11 Aggregate Summary

Supported:

- arrays inside structs with packed declaration-order layout
- nested struct fields with composed constant offsets
- nested aggregate initializer lists for arrays and structs
- string literal initialization of `char` / `unsigned char` array fields
- zero-fill for omitted nested array/struct elements
- designated struct-field initializers: `.field = value`
- designated array-index initializers: `[index] = value`
- whole-struct assignment between compatible complete struct types

Diagnostics:

- duplicate designated fields
- duplicate array designators
- unknown designated fields
- array designator indices that are non-constant or out of range
- too many initializer elements for arrays or structs
- self-containing structs by value
- incompatible struct assignment
- assignment to const aggregate objects
- whole-struct assignment inside interrupt handlers

## Phase 12 Pointer Summary

Supported:

- nested data-space pointer types including pointer-to-pointer values, parameters, and returns
- const-qualified pointer forms: `const T *`, `T * const`, and `const T * const`
- implicit conversion from `T *` to `const T *`
- pointer equality/inequality and relational comparisons for compatible data-space pointer types
- pointer subtraction for compatible data-space pointer types with 1-byte or 2-byte elements
- RAM-backed string literal objects that may initialize `char *` and `const char *`
- startup/listing comments that name string-literal symbols and pointer-valued static initializers
- map output that groups string-literal data symbols explicitly

Diagnostics:

- incompatible pointer assignments
- qualifier discard through pointers
- writes through pointer-to-const
- reassignment of const pointer objects
- incompatible pointer relational comparisons
- unsupported pointer subtraction element sizes
- incompatible string-literal pointer targets

## Phase 13 ROM Summary

Supported:

- file-scope `const __rom unsigned char[]` and `const __rom char[]`
- brace-list ROM table initializers
- string-literal ROM string initializers
- `__rom_read8(table, index)` over named ROM byte-array objects
- `.hex`, `.map`, and `.lst` output that shows ROM tables separately
- RETLW-backed ROM table lowering with one entry instruction plus one program word per byte

Diagnostics:

- non-const ROM objects
- local ROM objects
- direct ROM array indexing
- ROM/data-pointer mixing
- ROM pointer types
- ROM reads inside interrupt handlers
- unsupported ROM object types
- oversize ROM tables that do not fit one Phase 13 RETLW page

## Supported Subset

Supported:

- `#include`
- object-like `#define`
- `#if`, `#ifdef`, `#ifndef`, `#else`, `#endif`
- functions
- globals
- auto locals
- static locals
- file-scope `typedef` aliases for supported object/value types
- `enum` declarations and enumerator constants
- named packed `struct` declarations with nested struct and one-dimensional array fields
- file-scope `const __rom char[]` and `const __rom unsigned char[]`
- `if` / `else`
- `while`
- `for`
- `do while`
- `switch` / `case` / `default`
- `break` / `continue`
- `return`
- direct calls
- `char`, `unsigned char`, `int`, `unsigned int`, `void`
- fixed-size one-dimensional arrays of supported scalar types and complete named struct types
- omitted array size when inferred from a brace initializer list or string literal
- complete named struct objects with scalar, one-dimensional array, nested struct, or supported pointer fields
- `const` scalar, one-dimensional array, and complete named struct objects
- nested data-space pointers to supported scalar, pointer, or complete named struct types in PIC16 RAM
- `&obj`
- `*ptr`
- `a[i]`
- `p[i]`
- `.` and `->`
- unary `!`, `~`, unary `-`
- `+`, `-`, `*`, `/`, `%`, `<<`, `>>`, `&`, `|`, `^`
- `==`, `!=`, `<`, `<=`, `>`, `>=`
- compile-time `sizeof` for supported scalars, pointers, and fixed arrays
- `__rom_read8(table, index)` for program-memory byte arrays
- positional and designated array/struct initializer lists with zero-fill
- nested aggregate initializer lists
- string literal initialization for char/unsigned-char array fields and char/unsigned-char array fields inside structs
- RAM-backed string literal initialization of `char *` and `const char *`
- whole-struct assignment between compatible complete struct types
- explicit casts for supported scalar and data-pointer forms
- indirect data access through `FSR/INDF`
- 3+ argument stack calls
- stack-backed local arrays
- pointer arguments and pointer returns
- one `void __interrupt isr(void)` handler
- interrupt vector emission at `0x0004`
- `retfie`

Partially supported / constrained:

- `typedef` names are file-scope only and cannot conflict with object/function names
- enums use fixed 16-bit `int` representation in this phase
- structs use packed declaration-order layout with no implicit padding
- struct copy lowers byte-by-byte through ordinary indirect memory operations
- designated initializers support only single-step `.field` and `[index]` forms; chained designators are still deferred
- global aggregate initializer elements must be constant expressions
- explicit casts are limited to scalar conversions, data-pointer bitcasts, `(T*)0`, and pointer-to-16-bit-integer casts
- string literals become RAM-backed static data objects when used in pointer-initializer contexts; duplicate pooling is not attempted
- startup code writes initialized global/static bytes and clears zero-init storage; explicit `__rom` objects bypass startup and emit into program memory as RETLW tables
- plain `const` objects remain RAM-backed in this phase unless explicitly declared `__rom`
- `__rom_read8()` reads a named ROM byte-array object through a generated RETLW table call rather than a general code-space pointer model
- nested pointer qualifier conversions are intentionally conservative: exact nested qualifiers are required beyond one-level `T *` to `const T *`
- pointer subtraction assumes the compared pointers refer into the same object, matching ordinary C same-object expectations
- pointer subtraction supports only element sizes of 1 or 2 bytes in this phase
- local aggregate initializers and whole-struct copies remain rejected inside interrupt handlers
- ROM objects are limited to file-scope `char` / `unsigned char` arrays of at most 255 elements
- switch lowering uses linear compare chains; no jump tables are emitted in this phase
- case/default labels must stay in the switch body flow or nested blocks; labels under unrelated control statements like `if`, `while`, or `for` are rejected in phase 9

Unsupported:

- `union`
- source-level function pointers
- multidimensional arrays
- anonymous nested struct/enum fields without declarators
- chained designators such as `.outer.inner = 1`
- pointers to incomplete struct types
- program-memory / code-space pointers
- direct `rom_array[index]` syntax
- `float`
- recursion

Current constraints:

- recursion is rejected because Phase 4 computes maximum software-stack depth statically and has no runtime overflow checks
- returning a pointer to stack-local storage is rejected, including direct forms and obvious local alias chains
- explicit casts stay limited to scalar conversions, data-pointer bitcasts, `(T*)0`, and pointer-to-16-bit-integer casts
- aggregate initializers inside interrupt handlers remain rejected
- whole-struct copy inside interrupt handlers remains rejected
- global aggregate initializer elements must be constant expressions
- string literal array initializers are accepted only for `char` / `unsigned char` arrays, and explicit array sizes must fit the trailing null byte too
- string literals that initialize pointers become anonymous RAM-backed static objects; map output groups them under string literals
- globals, file-scope statics, and static locals are initialized by startup code in RAM, not by a separate read-only data segment
- plain const RAM data remains in startup-initialized RAM; explicit `__rom` data uses separate RETLW-backed program-memory tables
- nested data-space pointer comparisons are raw address-order comparisons in RAM
- implicit nested-pointer qualifier changes beyond `T *` to `const T *` are rejected conservatively
- pointer subtraction is limited to compatible pointer types whose element size is 1 or 2 bytes
- ROM reads use `__rom_read8()` only; general ROM address values and ROM pointers are still unsupported
- switch expressions must stay in the supported integer subset; case labels must be constant and representable
- switch inside ISR is allowed only when the controlling expression and body remain inline-safe under existing Phase 6 helper restrictions
- reads from global/static/const RAM objects are allowed inside ISR when the resulting expressions stay inline-safe
- pointers are data-space-only; code pointers remain unsupported
- only one ISR is supported in this phase
- ISR code cannot call normal functions or Phase 5 runtime helpers
- no emulator or hardware execution runs in CI; validation is compile/listing/map/HEX shape based

## Known Limitations (Phase 13)

- recursion is unsupported
- no runtime software-stack overflow detection is implemented
- ISR restrictions remain conservative: one ISR, no normal calls, no runtime-helper-requiring expressions
- aggregate support is still intentionally constrained: no multidimensional arrays, no anonymous nested fields, no chained designators, and no incomplete-struct pointers
- switch lowering stays intentionally simple: compare chains only, no jump tables, and labels nested under other control statements are rejected
- automatic string literals used as pointers are still RAM-backed static data objects; there is no automatic ROM string pooling pass
- only explicit `const __rom` byte arrays use program memory; there is still no general ROM pointer or code-space string-pointer model
- implicit nested-pointer qualifier conversions stay conservative beyond one-level `T *` to `const T *`
- enums stay fixed to 16-bit `int`; structs stay packed with no padding
- `union` and `float` are unsupported
- source-level function pointers are unsupported
- dynamic division/modulo by zero returns `0` (constant zero divisors are diagnostics)
- pointers are data-space only; code pointers are unsupported
- pointer subtraction is intentionally limited to 1-byte and 2-byte element types
- multidimensional arrays are unsupported

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
- [examples/pic16f628a/array_initializer.c](/home/settes/cursus/PIC16_compiler/examples/pic16f628a/array_initializer.c:1)
- [examples/pic16f628a/casts.c](/home/settes/cursus/PIC16_compiler/examples/pic16f628a/casts.c:1)
- [examples/pic16f628a/pointer_to_pointer.c](/home/settes/cursus/PIC16_compiler/examples/pic16f628a/pointer_to_pointer.c:1)
- [examples/pic16f628a/rom_table.c](/home/settes/cursus/PIC16_compiler/examples/pic16f628a/rom_table.c:1)
- [examples/pic16f628a/stack_abi.c](/home/settes/cursus/PIC16_compiler/examples/pic16f628a/stack_abi.c:1)
- [examples/pic16f628a/string_array.c](/home/settes/cursus/PIC16_compiler/examples/pic16f628a/string_array.c:1)
- [examples/pic16f628a/struct_array_field.c](/home/settes/cursus/PIC16_compiler/examples/pic16f628a/struct_array_field.c:1)
- [examples/pic16f628a/struct_initializer.c](/home/settes/cursus/PIC16_compiler/examples/pic16f628a/struct_initializer.c:1)
- [examples/pic16f628a/struct_point.c](/home/settes/cursus/PIC16_compiler/examples/pic16f628a/struct_point.c:1)
- [examples/pic16f628a/switch_state.c](/home/settes/cursus/PIC16_compiler/examples/pic16f628a/switch_state.c:1)
- [examples/pic16f628a/typedef_enum.c](/home/settes/cursus/PIC16_compiler/examples/pic16f628a/typedef_enum.c:1)
- [examples/pic16f877a/blink.c](/home/settes/cursus/PIC16_compiler/examples/pic16f877a/blink.c:1)
- [examples/pic16f877a/call_chain.c](/home/settes/cursus/PIC16_compiler/examples/pic16f877a/call_chain.c:1)
- [examples/pic16f877a/compare16.c](/home/settes/cursus/PIC16_compiler/examples/pic16f877a/compare16.c:1)
- [examples/pic16f877a/const_pointers.c](/home/settes/cursus/PIC16_compiler/examples/pic16f877a/const_pointers.c:1)
- [examples/pic16f877a/config_table.c](/home/settes/cursus/PIC16_compiler/examples/pic16f877a/config_table.c:1)
- [examples/pic16f877a/div16.c](/home/settes/cursus/PIC16_compiler/examples/pic16f877a/div16.c:1)
- [examples/pic16f877a/designated_init.c](/home/settes/cursus/PIC16_compiler/examples/pic16f877a/designated_init.c:1)
- [examples/pic16f877a/expression_test.c](/home/settes/cursus/PIC16_compiler/examples/pic16f877a/expression_test.c:1)
- [examples/pic16f877a/const_config.c](/home/settes/cursus/PIC16_compiler/examples/pic16f877a/const_config.c:1)
- [examples/pic16f877a/global_init.c](/home/settes/cursus/PIC16_compiler/examples/pic16f877a/global_init.c:1)
- [examples/pic16f877a/mod16.c](/home/settes/cursus/PIC16_compiler/examples/pic16f877a/mod16.c:1)
- [examples/pic16f877a/mul16.c](/home/settes/cursus/PIC16_compiler/examples/pic16f877a/mul16.c:1)
- [examples/pic16f877a/nested_struct.c](/home/settes/cursus/PIC16_compiler/examples/pic16f877a/nested_struct.c:1)
- [examples/pic16f877a/pointer16.c](/home/settes/cursus/PIC16_compiler/examples/pic16f877a/pointer16.c:1)
- [examples/pic16f877a/pointer_compare.c](/home/settes/cursus/PIC16_compiler/examples/pic16f877a/pointer_compare.c:1)
- [examples/pic16f877a/pointer_subtract.c](/home/settes/cursus/PIC16_compiler/examples/pic16f877a/pointer_subtract.c:1)
- [examples/pic16f877a/rom_lookup.c](/home/settes/cursus/PIC16_compiler/examples/pic16f877a/rom_lookup.c:1)
- [examples/pic16f877a/rom_string.c](/home/settes/cursus/PIC16_compiler/examples/pic16f877a/rom_string.c:1)
- [examples/pic16f877a/shift_mix.c](/home/settes/cursus/PIC16_compiler/examples/pic16f877a/shift_mix.c:1)
- [examples/pic16f877a/static_table.c](/home/settes/cursus/PIC16_compiler/examples/pic16f877a/static_table.c:1)
- [examples/pic16f877a/struct_copy.c](/home/settes/cursus/PIC16_compiler/examples/pic16f877a/struct_copy.c:1)
- [examples/pic16f877a/string_pointer.c](/home/settes/cursus/PIC16_compiler/examples/pic16f877a/string_pointer.c:1)
- [examples/pic16f877a/switch_enum.c](/home/settes/cursus/PIC16_compiler/examples/pic16f877a/switch_enum.c:1)
- [examples/pic16f877a/switch_fallthrough.c](/home/settes/cursus/PIC16_compiler/examples/pic16f877a/switch_fallthrough.c:1)
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
- [docs/backend/phase10-data-layout.md](/home/settes/cursus/PIC16_compiler/docs/backend/phase10-data-layout.md:1)
- [docs/backend/phase11-aggregate-copy.md](/home/settes/cursus/PIC16_compiler/docs/backend/phase11-aggregate-copy.md:1)
- [docs/backend/phase12-string-pointer-data.md](/home/settes/cursus/PIC16_compiler/docs/backend/phase12-string-pointer-data.md:1)
- [docs/backend/phase13-rom-data-layout.md](/home/settes/cursus/PIC16_compiler/docs/backend/phase13-rom-data-layout.md:1)
- [docs/backend/phase9-switch-codegen.md](/home/settes/cursus/PIC16_compiler/docs/backend/phase9-switch-codegen.md:1)
- [docs/ir/phase4-call-lowering.md](/home/settes/cursus/PIC16_compiler/docs/ir/phase4-call-lowering.md:1)
- [docs/backend/phase5-helper-calling.md](/home/settes/cursus/PIC16_compiler/docs/backend/phase5-helper-calling.md:1)
- [docs/ir/phase5-arithmetic-lowering.md](/home/settes/cursus/PIC16_compiler/docs/ir/phase5-arithmetic-lowering.md:1)
- [docs/ir/phase10-static-initializers.md](/home/settes/cursus/PIC16_compiler/docs/ir/phase10-static-initializers.md:1)
- [docs/ir/phase11-aggregate-initializers.md](/home/settes/cursus/PIC16_compiler/docs/ir/phase11-aggregate-initializers.md:1)
- [docs/ir/phase12-pointer-lowering.md](/home/settes/cursus/PIC16_compiler/docs/ir/phase12-pointer-lowering.md:1)
- [docs/ir/phase13-rom-lowering.md](/home/settes/cursus/PIC16_compiler/docs/ir/phase13-rom-lowering.md:1)
- [docs/ir/phase9-switch-lowering.md](/home/settes/cursus/PIC16_compiler/docs/ir/phase9-switch-lowering.md:1)
- [docs/frontend/phase10-string-literals.md](/home/settes/cursus/PIC16_compiler/docs/frontend/phase10-string-literals.md:1)
- [docs/frontend/phase11-aggregates.md](/home/settes/cursus/PIC16_compiler/docs/frontend/phase11-aggregates.md:1)
- [docs/frontend/phase12-pointers.md](/home/settes/cursus/PIC16_compiler/docs/frontend/phase12-pointers.md:1)
- [docs/frontend/phase13-rom-address-space.md](/home/settes/cursus/PIC16_compiler/docs/frontend/phase13-rom-address-space.md:1)
- [docs/frontend/phase9-switch.md](/home/settes/cursus/PIC16_compiler/docs/frontend/phase9-switch.md:1)
- [docs/runtime/phase5-arithmetic-helpers.md](/home/settes/cursus/PIC16_compiler/docs/runtime/phase5-arithmetic-helpers.md:1)
- [docs/migration/phase3-to-phase4-abi.md](/home/settes/cursus/PIC16_compiler/docs/migration/phase3-to-phase4-abi.md:1)
- [docs/developer-guide/adding-device.md](/home/settes/cursus/PIC16_compiler/docs/developer-guide/adding-device.md:1)

## License

- compiler source, tests, docs, examples, and scripts: `GPL-3.0-or-later`
- public headers and runtime material intended for compiled firmware: `GPL-3.0-or-later WITH GCC-exception-3.1`
- [COPYING](/home/settes/cursus/PIC16_compiler/COPYING:1) contains the full GNU GPLv3 text
- [COPYING.RUNTIME](/home/settes/cursus/PIC16_compiler/COPYING.RUNTIME:1) contains the GCC Runtime Library Exception 3.1 text

## Current Limits

Phase 13 adds explicit `const __rom` byte arrays, RETLW-backed program-memory tables, `__rom_read8()` ROM reads, and separate ROM map/listing output. Current hard limits remain: no general ROM pointer model, no code-space pointers, no jump tables, no case/default labels buried under other control statements, no `union`, no multidimensional arrays, no anonymous nested aggregate fields, no chained designators, no incomplete-struct pointers, no function pointers, no `float`, and no recursion.
