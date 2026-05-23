# Phase 42 Float Compare Compaction

Phase 42 replaces the old dynamic f32 comparison path with a compact finite-only raw-order helper.

## Helper

```text
__rt_f32_cmp(a, b)
```

Arguments are two 32-bit little-endian raw f32 values passed through the Stack-first ABI. The helper returns one byte in `W`:

```text
0x00  a == b
0x01  a > b
0xff  a < b
```

The helper no longer converts both operands through Q16.16. It compares raw IEEE f32 sign/exponent/mantissa fields:

- byte-exact equality returns equal
- different signs order negative finite values before positive finite values
- same-sign positive values use raw unsigned ordering
- same-sign negative values use reversed raw unsigned ordering

## Limits

This remains the compiler finite-float model:

- no NaN ordering
- no Infinity ordering
- no errno or fenv
- no full IEEE signed-zero guarantee

`+0.0f == +0.0f` is validated. Mixed `+0.0f` / `-0.0f` tie behavior is implementation-defined within the finite-only model.

## Reports

`__rt_f32_cmp` remains a `float` runtime helper in `--size`, `--memory-report`, `.map`, and `.lst`. Phase 42 tests assert the emitted helper is substantially smaller than the Phase 41 Q16.16 conversion-based helper.
