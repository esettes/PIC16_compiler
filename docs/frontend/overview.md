<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Frontend

Phase 24 frontend support accepts dynamic Q16.16/UQ16.16 multiply and divide expressions instead of rejecting them. Phase 23 fixed-point decimal literals and ROM calibration tables remain supported on top of the Phase 22 scalar types `__fixed8_8`, `__ufixed8_8`, `__fixed16_16`, and `__ufixed16_16`. Raw constructors `__q8_8(raw)`, `__uq8_8(raw)`, `__q16_16(raw)`, and `__uq16_16(raw)` remain available.

Phase 21 frontend support remains: `long`, `signed long`, `unsigned long`, and integer suffixes `U`, `L`, `UL`, and `LU`. The semantic model warns on implicit narrowing from 32-bit values and rejects literals outside the supported 32-bit range.

See `docs/frontend/phase23-fixed-literals.md`, `docs/frontend/phase22-fixed-point.md`, and `docs/frontend/phase21-long-types.md` for exact type, conversion, ISR, and ROM rules.

Responsibilities:

- preprocess
- tokenize
- parse
- build the AST
- resolve symbols and types

Output:

- a typed program ready for IR lowering

Current Phase 18 frontend surface:

- parses `typedef`, `enum`, and named packed `struct` declarations
- parses named packed `union` declarations and basic unsigned bitfield fields
- records enum constants plus struct/union/bitfield layout metadata in the typed frontend model
- lowers `.` / `->` member access through typed base + offset expressions
- accepts nested array/struct initializer lists with zero-fill
- accepts designated initializers for `.field` and `[index]`
- accepts arrays inside structs and nested struct fields
- accepts named unions inside structs and structs/unions inside unions when complete
- accepts first-field and designated union initializers with whole-storage zero-fill
- accepts whole-struct and whole-union assignment between compatible complete types
- lowers bitfield reads/writes as real lvalues instead of pretending they are plain byte objects
- accepts fixed-size multidimensional RAM arrays with row-major layout
- accepts repeated indexing like `matrix[i][j]`
- accepts chained designated initializers across mixed `.field` / `[index]` paths
- parses string literals with the supported escape subset
- infers omitted array sizes from supported brace or string initializers
- accepts string literal initialization for `char` / `unsigned char` arrays
- accepts string literal initialization for `char` / `unsigned char` array fields inside structs
- materializes RAM-backed string literal objects when one pointer initializer needs an addressable literal
- accepts pointer-to-pointer types and const-qualified pointer forms in the data-space pointer model
- rejects writes to const objects and through pointer-to-const
- rejects reassignment of const pointer objects and implicit qualifier discard
- validates explicit casts for supported scalar/data-pointer combinations
- accepts fixed-point casts between integer/fixed formats with explicit scaling
- rejects implicit unsafe fixed narrowing under `-Werror`
- rejects fixed bitwise operations unless the program casts to a raw integer type first
- accepts fixed decimal literals with `q8_8`, `uq8_8`, `q16_16`, and `uq16_16` suffixes
- accepts one-dimensional fixed-point `const __rom` arrays for direct indexing
- folds Q16.16 fixed multiply/divide constants and accepts dynamic Q16.16 helper-backed multiply/divide outside ISRs
- accepts `float` declarations, finite decimal float literals, and constant float casts/comparisons/arithmetic
- accepts dynamic integer/fixed-point float casts through helper lowering
- accepts Phase 29 dynamic float comparisons
- accepts Phase 30 file-scope `const __rom float[]` tables with direct indexing
- accepts Phase 36/37/41/42/43/44/48/49 `math.h` declarations for `fabsf`, `truncf`, `floorf`, `ceilf`, `roundf`, `sqrtf`, `fminf`, `fmaxf`, `sinf`, `cosf`, and `tanf`
- accepts Phase 38/39 math profile selection through CLI; source syntax is unchanged
- folds supported math calls with finite constant float arguments
- rejects implicit mixed float/integer arithmetic, float bitfields, float switch expressions, ROM/data pointer mixing, and helper-backed float work in ISRs
- validates pointer relational comparisons for compatible data-space pointer types
- validates pointer subtraction for compatible data-space pointer types with 1-byte or 2-byte elements
- parses explicit `__rom` declarations for file-scope 8-bit/16-bit integer arrays
- accepts `const __rom char[]`, `const __rom unsigned char[]`, `const __rom int[]`, `const __rom unsigned int[]`, fixed-point ROM arrays, and `const __rom float[]` initializers from brace lists or string literals where applicable
- rejects ROM/data-pointer mixing and rejects ROM pointer forms
- lowers direct ROM indexing plus `__rom_read8(table, index)` / `__rom_read16(table, index)` as the supported ROM read surfaces
- parses supported function-pointer declarators, typedefs, arrays, and struct fields
- validates supported zero-arg/one-arg integer signatures for function-pointer objects
- lowers bare function names and `&function` as function-pointer values
- accepts compatible function-pointer equality/inequality and indirect calls in normal code
- rejects function-pointer arithmetic, relational comparisons, pointer-to-function-pointer objects, and indirect calls inside ISR
- detects direct and mutual recursion cycles with Phase 18 diagnostics that mention stack-check behavior
- parses `switch`, `case`, and `default`
- validates case-label constants, duplicate cases, default count, and switch expression types
- preserves C-style fallthrough and innermost-construct `break` behavior for supported switch forms
- rejects `case` / `default` labels nested under unrelated control statements in phase 9
- still enforces Phase 6 ISR restrictions on helper-requiring code paths and rejects local aggregate init / whole-aggregate copy inside ISRs

Current detail:

- [phase6-isr-syntax.md](phase6-isr-syntax.md)
- [phase8-types.md](phase8-types.md)
- [phase9-switch.md](phase9-switch.md)
- [phase10-string-literals.md](phase10-string-literals.md)
- [phase11-aggregates.md](phase11-aggregates.md)
- [phase12-pointers.md](phase12-pointers.md)
- [phase13-rom-address-space.md](phase13-rom-address-space.md)
- [phase14-rom-indexing.md](phase14-rom-indexing.md)
- [phase15-union-bitfields.md](phase15-union-bitfields.md)
- [phase16-multidimensional-arrays.md](phase16-multidimensional-arrays.md)
- [phase17-function-pointers.md](phase17-function-pointers.md)
- [phase21-long-types.md](phase21-long-types.md)
- [phase22-fixed-point.md](phase22-fixed-point.md)
- [phase23-fixed-literals.md](phase23-fixed-literals.md)
- [phase27-float.md](phase27-float.md)
- [phase28-float-conversions.md](phase28-float-conversions.md)
