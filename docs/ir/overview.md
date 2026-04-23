# IR

Custom CFG-based IR.

Motivation:

- separate the AST from the backend
- enable validation and optimization passes
- avoid rewriting the frontend as the backend grows

Phase 3 adds typed casts, typed compare conditions, explicit address materialization, and explicit indirect memory operations so pointer and array lowering stays explicit.

Detail: [phase2-lowering.md](/home/settes/cursus/PIC16_compiler/docs/ir/phase2-lowering.md:1) and [phase3-memory.md](/home/settes/cursus/PIC16_compiler/docs/ir/phase3-memory.md:1)
