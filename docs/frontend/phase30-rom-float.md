# Phase 30 ROM Float Frontend

Supported source form:

```c
const __rom float calibration[] = {
    1.0f,
    1.5f,
    2.0f
};
```

Rules:

- ROM float objects are file-scope only.
- `const` is required; plain `__rom float[]` is rejected.
- Initializers must be brace lists of finite float constants or already-foldable float expressions.
- Omitted outer array size is inferred from the initializer list.
- Direct indexing is supported: `calibration[index]`.
- ROM arrays do not decay to data pointers.
- `&calibration[0]`, assignment to `calibration[0]`, and ROM/data pointer mixing are rejected.
- Local ROM float arrays are rejected.
- Dynamic ROM reads inside ISR are rejected; constant-index reads are allowed only when they stay inline-safe.

There is no general ROM pointer model and no `__rom_read_float` builtin in Phase 30. Direct `table[index]` is the user-facing API.
