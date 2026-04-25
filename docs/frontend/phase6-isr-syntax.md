# Phase 6 ISR Syntax

Phase status:

- ISR syntax and restrictions are frozen at Phase 6 in this branch
- no additional interrupt syntax variants are planned here

Chosen syntax:

```c
void __interrupt isr(void) {
    /* ... */
}
```

Other interrupt syntaxes are not implemented in this phase.

Current Phase 6 rules:

- exactly one ISR per program
- return type must be `void`
- parameter list must be `void` / empty
- ISR name is arbitrary; `isr` is only a convention
- ISR must be defined, not only declared

Current diagnostics:

- non-`void` ISR is rejected
- ISR with parameters is rejected
- multiple `__interrupt` functions are rejected
- returning a value from ISR is rejected
- normal function calls inside ISR are rejected
- Phase 5 runtime-helper-requiring expressions inside ISR are rejected

Current allowed ISR subset:

- direct SFR reads/writes
- globals
- stack-backed locals and IR temps
- `if`, loops, compares
- inline-safe arithmetic and bitwise ops
- pointer dereference and array indexing that lower inline

Current disallowed ISR subset:

- normal function calls
- runtime-helper calls
- helper-requiring `*`, `/`, `%`
- helper-requiring dynamic `<<` / `>>`
