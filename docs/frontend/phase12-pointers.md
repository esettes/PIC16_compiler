# Phase 12 Pointers

Phase 12 extends the existing PIC16 RAM-only pointer model without introducing code-space pointers.

Supported:

- pointer-to-pointer types in declarations, locals, globals, parameters, and returns
- dereference chains such as `**pp`
- address-of pointer variables such as `pp = &p`
- const-qualified pointer forms:
  - `const T *`
  - `T * const`
  - `const T * const`
- pointer equality, inequality, and relational comparisons for compatible data-space pointer types
- pointer subtraction for compatible data-space pointer types whose element size is 1 or 2 bytes
- string literals used as RAM-backed pointer initializers for `char *` and `const char *`

Rules:

- pointers remain 16-bit RAM addresses only
- `T *` to `const T *` is the only implicit qualifier-adding pointer conversion accepted in this phase
- deeper nested-pointer qualifier changes require an explicit cast and otherwise diagnose conservatively
- writing through pointer-to-const is rejected
- reassigning a const pointer object is rejected
- pointer relational comparisons are raw address-order comparisons in PIC16 data memory
- pointer subtraction assumes the pointers refer into the same object, matching ordinary C same-object expectations

String literals:

- each literal becomes one anonymous RAM-backed static byte array with a trailing null byte
- pointer initializers store the address of that static object
- duplicate pooling is not attempted in this phase

ISR notes:

- pointer reads, pointer compares, and pointer-to-pointer operations are allowed when they lower inline safely
- pointer subtraction is allowed only for the inline 1-byte / 2-byte element-size cases
- code-space pointer models remain unsupported
