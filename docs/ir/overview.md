# IR

Custom CFG-based IR.

Motivation:

- separate the AST from the backend
- enable validation and optimization passes
- avoid rewriting the frontend as the backend grows

Phase 2 adds typed casts and typed compare conditions so 16-bit lowering stays explicit.

Detail: [phase2-lowering.md](/home/settes/cursus/PIC16_compiler/docs/ir/phase2-lowering.md:1)
