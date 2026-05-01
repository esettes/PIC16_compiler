<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Phase 10: Frontend String Literals and Const Data

Phase 10 adds frontend support for common firmware-style static data patterns without changing the one-address-space PIC16 data-pointer model.

String literal syntax:

- ordinary quoted strings like `"OK"` and `"hello"`
- supported escapes: `\n`, `\r`, `\t`, `\\`, `\"`, `\0`
- hexadecimal escapes are rejected with a clear diagnostic in this phase
- every parsed string literal carries one trailing null byte

Supported uses:

- `char msg[] = "OK";`
- `unsigned char msg[] = "OK";`
- `char msg[3] = "OK";`
- omitted array size inferred from string length plus trailing null
- omitted array size inferred from brace initializer element count for supported array declarations

String-array initialization rules:

- only `char` and `unsigned char` arrays may initialize from string literals
- explicit array sizes must fit the whole string including the trailing null byte
- missing bytes after the string payload are zero-filled
- `char msg[2] = "OK";` is rejected in this phase because the null byte would not fit

Unsupported string uses:

- standalone string literal expressions in arithmetic, assignment, switch, or return contexts
- pointer initialization from string literals
- standalone pooled string objects or code-space string pointers

Const model:

- `const` scalar, array, and flat-struct objects are supported
- assignment to const objects is rejected semantically
- writes to fields of const struct objects are rejected
- const-qualified pointer forms are rejected in this phase because the type model only has one data-space pointer kind
- const storage still lives in RAM, not program memory

Global/static initialization:

- globals, file-scope statics, and static locals accept constant scalar initializers
- array and flat-struct initializers remain positional with zero-fill
- string-initialized char arrays participate in the same static-initializer path

ISR interaction:

- reading global/static/const RAM objects from ISR is allowed when the resulting code remains inline-safe under the existing Phase 6 rules
- string literals are not usable directly inside ISR expressions because standalone string literal expressions are unsupported
<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
