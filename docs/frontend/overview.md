# Frontend

Responsibilities:

- preprocess
- tokenize
- parse
- build the AST
- resolve symbols and types

Output:

- a typed program ready for IR lowering

Current Phase 6 frontend additions:

- parses `void __interrupt isr(void)`
- records one interrupt-function marker in semantic symbols
- rejects ISR signature mismatches
- rejects multiple ISR declarations
- rejects normal calls and runtime-helper-requiring expressions inside ISR bodies

Current detail:

- [phase6-isr-syntax.md](/home/settes/cursus/PIC16_compiler/docs/frontend/phase6-isr-syntax.md:1)
