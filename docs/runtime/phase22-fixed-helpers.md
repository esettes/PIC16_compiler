<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Phase 22 Fixed Helpers

Phase 22 does not add IEEE float helpers. Fixed-point Q8.8 multiply/divide reuse raw 32-bit integer helper paths from Phase 21.

Q8.8 multiply:

```text
raw_result = (raw_a_32 * raw_b_32) >> 8
```

Q8.8 divide:

```text
raw_result = (raw_a_32 << 8) / raw_b_32
```

Signed Q8.8 sign-extends raw 16-bit operands to signed 32-bit before helper use. UQ8.8 zero-extends raw operands.

The Stack-first ABI remains unchanged:

- Q8.8 values occupy two argument/return bytes at the C ABI boundary
- widened helper intermediates occupy four bytes inside lowered helper expressions
- returned raw fixed results truncate back to 16-bit before storing Q8.8/UQ8.8 values

Deferred:

- Q16.16 multiply/divide
- wider-than-32-bit helper intermediates
- fixed-point ROM tables
- decimal fixed literal parser
