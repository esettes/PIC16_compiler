# Phase 8: Frontend Type Features

Phase 8 extends the frontend with a small, well-scoped set of C features:

- `typedef` aliases at file scope for scalars, pointers, arrays, and named structs
- `enum` declarations with implicit and explicit enumerator values
- named `struct` declarations with packed field layout (no padding)
- field access via `.` and `->` lowered to pointer base + offset
- positional brace initializers for arrays and structs
- explicit C-style casts for allowed scalar/pointer combinations

Notes and restrictions:

- nested structs or arrays inside struct fields are rejected in this phase
- designated initializers (`.field = ...`) are rejected and cause a clear diagnostic
- whole-struct assignment (`a = b` where `a` and `b` are structs) is rejected
- enum representation is 16-bit integer for now
- typedefs must not conflict with object/function names
