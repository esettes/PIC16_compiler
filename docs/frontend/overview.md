# Frontend

Responsibilities:

- preprocess
- tokenize
- parse
- build the AST
- resolve symbols and types

Output:

- a typed program ready for IR lowering

Current Phase 8 frontend surface:

- parses `typedef`, `enum`, and named packed `struct` declarations
- records enum constants and struct layout metadata in the typed frontend model
- lowers `.` / `->` member access through typed base + offset expressions
- accepts flat positional array/struct initializer lists with zero-fill
- validates explicit casts for supported scalar/data-pointer combinations
- still enforces Phase 6 ISR restrictions on helper-requiring or aggregate-heavy code paths

Current detail:

- [phase6-isr-syntax.md](/home/settes/cursus/PIC16_compiler/docs/frontend/phase6-isr-syntax.md:1)
- [phase8-types.md](/home/settes/cursus/PIC16_compiler/docs/frontend/phase8-types.md:1)
