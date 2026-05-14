# Phase 29 Float Comparison Lowering

Dynamic finite-float comparisons lower to the shared backend helper path:

```text
__rt_f32_cmp(a, b)
```

The helper returns:

- `0xFF` for `a < b`
- `0x00` for `a == b`
- `0x01` for `a > b`

IR keeps the original comparison operator. Backend branch lowering maps helper result to `==`, `!=`, `<`, `<=`, `>`, and `>=` boolean behavior.

The result type is normalized `unsigned char` (`0` or `1`) for value contexts. Control-flow conditions use the same helper result without exposing a public compare type.

Limits:

- finite values only
- NaN/Inf unsupported
- helper-backed comparisons rejected inside ISR
