# Phase 29 Float / 32-bit Integer Conversions

Phase 29 adds runtime helpers for dynamic 32-bit integer and float casts:

- `__rt_i32_to_f32`
- `__rt_u32_to_f32`
- `__rt_f32_to_i32`
- `__rt_f32_to_u32`

ABI:

- all arguments are 4 bytes
- all returns are 4 bytes
- `float` values use little-endian IEEE f32 storage bits
- 32-bit integer values use Phase 21 little-endian `long` / `unsigned long`

Conversion policy:

- integer to float normalizes the integer magnitude and packs finite f32 bits
- float to integer truncates toward zero
- dynamic negative float to `unsigned long` returns `0`
- dynamic out-of-range float to integer uses the finite helper's 32-bit shift/truncation behavior
- constant out-of-range float-to-`long` / `unsigned long` casts are diagnostics

These helpers are finite-only. They do not implement NaN/Inf semantics and do not add `double`.
