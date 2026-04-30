# Phase 10: IR Static Initializers

Phase 10 keeps string/static-data support out of the backend AST path and out of the general IR expression model.

Chosen strategy:

- frontend parses string literals as null-terminated byte strings
- semantic analysis consumes supported string literals only when initializing `char` or `unsigned char` arrays
- omitted array sizes are resolved before symbol layout is fixed
- global, file-scope static, and static-local aggregate initializers become byte payloads
- scalar global/static initializers remain constant typed expressions
- automatic local aggregate initializers still lower to ordinary per-slot stores in the function body

IR consequences:

- no dedicated IR string literal instruction is introduced
- no dedicated IR static-data section object is introduced
- startup initialization stays modeled by backend startup-store emission from semantic payloads
- static locals reuse the same startup initializer data path as globals

Why keep it this way:

- it preserves the existing frontend -> IR -> backend layering
- PIC16 startup writes were already the mechanism for aggregate initialization
- there is no need to introduce code-space string pointers or a second address-space model yet
