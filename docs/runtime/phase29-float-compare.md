# Phase 29 Float Compare Helper

Phase 29 adds shared dynamic finite-float comparison helper:

```text
__rt_f32_cmp
```

ABI:

- argument 0: 32-bit raw `float`
- argument 1: 32-bit raw `float`
- return: `W`

Return values:

- `0xFF` when `a < b`
- `0x00` when `a == b`
- `0x01` when `a > b`

Supported lowering:

- `==`
- `!=`
- `<`
- `<=`
- `>`
- `>=`
- assignment to `unsigned char`
- `if`, `while`, and `for` conditions

Policy:

- finite values only
- `+0.0f` and `-0.0f` both compare as zero
- NaN/Inf behavior is unsupported
- helper-backed comparisons are rejected inside ISRs

Implementation note: the helper compares through the same finite Q16.16 work format used by the current float runtime. Values outside that practical embedded range are not full IEEE comparisons.
