<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Phase 21 Long Types

Phase 21 adds controlled 32-bit integer support:

- `long` and `signed long` are signed 32-bit values.
- `unsigned long` is an unsigned 32-bit value.
- Storage is little-endian: byte 0 is least significant, byte 3 is most significant.
- `sizeof(long)` and `sizeof(unsigned long)` are 4.

Supported declarations include globals, statics, locals, parameters, returns, arrays, structs, unions, and data pointers to long objects. Literal suffixes accepted by the lexer are `U`, `L`, `UL`, and `LU`; values above the supported 32-bit range are rejected.

Conversions follow the existing warning model. Implicit narrowing from 32-bit to 16-bit or 8-bit warns, and fails under `-Werror`. Explicit casts suppress that narrowing warning.

ROM long tables are intentionally deferred. `const __rom unsigned long[]` is rejected with a clear diagnostic.
