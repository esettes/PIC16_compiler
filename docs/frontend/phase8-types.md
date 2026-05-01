<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Phase 8: Frontend Type Features

Phase 8 extends the frontend with a small, well-scoped set of C features:

- `typedef` aliases at file scope for scalars, pointers, arrays, and named structs
- `enum` declarations with implicit and explicit enumerator values
- named `struct` declarations with packed field layout (no padding)
- field access via `.` and `->` lowered to pointer base + offset
- positional brace initializers for arrays and structs
- explicit C-style casts for allowed scalar/pointer combinations

Notes and restrictions:

- typedef aliases are file-scope only; block-scope typedef declarations are rejected
- typedef names must not conflict with object/function names
- enum representation is fixed to 16-bit `int` in this phase
- named structs must stay flat: scalar or one-level pointer fields only
- nested structs or arrays inside struct fields are rejected in this phase
- array initializers apply to one-dimensional arrays of supported scalar targets
- struct initializers apply to flat named structs and use positional field order
- missing aggregate initializer elements are zero-filled
- global aggregate initializer elements must be constant expressions
- designated initializers (`.field = ...`) are rejected with a clear diagnostic
- nested initializer lists are rejected unless they wrap a single scalar element
- whole-struct assignment (`a = b` where `a` and `b` are structs) is rejected
- explicit casts support scalar conversions, one-level data-pointer bitcasts, `(T*)0`, and pointer-to-16-bit-integer casts
- pointer-to-pointer types remain unsupported in this phase
<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
