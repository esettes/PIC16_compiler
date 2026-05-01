# Frontend

Responsibilities:

- preprocess
- tokenize
- parse
- build the AST
- resolve symbols and types

Output:

- a typed program ready for IR lowering

Current Phase 12 frontend surface:

- parses `typedef`, `enum`, and named packed `struct` declarations
- records enum constants and struct layout metadata in the typed frontend model
- lowers `.` / `->` member access through typed base + offset expressions
- accepts nested array/struct initializer lists with zero-fill
- accepts designated initializers for `.field` and `[index]`
- accepts arrays inside structs and nested struct fields
- accepts whole-struct assignment between compatible complete struct types
- parses string literals with the supported escape subset
- infers omitted array sizes from supported brace or string initializers
- accepts string literal initialization for `char` / `unsigned char` arrays
- accepts string literal initialization for `char` / `unsigned char` array fields inside structs
- materializes RAM-backed string literal objects when one pointer initializer needs an addressable literal
- accepts pointer-to-pointer types and const-qualified pointer forms in the data-space pointer model
- rejects writes to const objects and through pointer-to-const
- rejects reassignment of const pointer objects and implicit qualifier discard
- validates explicit casts for supported scalar/data-pointer combinations
- validates pointer relational comparisons for compatible data-space pointer types
- validates pointer subtraction for compatible data-space pointer types with 1-byte or 2-byte elements
- parses `switch`, `case`, and `default`
- validates case-label constants, duplicate cases, default count, and switch expression types
- preserves C-style fallthrough and innermost-construct `break` behavior for supported switch forms
- rejects `case` / `default` labels nested under unrelated control statements in phase 9
- still enforces Phase 6 ISR restrictions on helper-requiring code paths and rejects local aggregate init / whole-struct copy inside ISRs

Current detail:

- [phase6-isr-syntax.md](/home/settes/cursus/PIC16_compiler/docs/frontend/phase6-isr-syntax.md:1)
- [phase8-types.md](/home/settes/cursus/PIC16_compiler/docs/frontend/phase8-types.md:1)
- [phase9-switch.md](/home/settes/cursus/PIC16_compiler/docs/frontend/phase9-switch.md:1)
- [phase10-string-literals.md](/home/settes/cursus/PIC16_compiler/docs/frontend/phase10-string-literals.md:1)
- [phase11-aggregates.md](/home/settes/cursus/PIC16_compiler/docs/frontend/phase11-aggregates.md:1)
- [phase12-pointers.md](/home/settes/cursus/PIC16_compiler/docs/frontend/phase12-pointers.md:1)
