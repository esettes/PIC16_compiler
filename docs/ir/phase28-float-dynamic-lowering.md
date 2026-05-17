# Phase 28 Float Dynamic Lowering

Phase 28 adds explicit cast IR for the finite float bridge:

- `Q16ToF32`
- `F32ToQ16`

Frontend lowering routes fixed-point dynamic casts through Q16.16 or UQ16.16 first, then emits one of these cast nodes.

Examples:

```c
int i;
float f;

f = (float)i;
i = (int)f;
```

Lowering shape:

```text
q8/q16 -> Q16.16 -> Q16ToF32
F32ToQ16 -> Q16.16 -> q8/q16
```

Phase 30.5 repaired dynamic integer/float casts so integer operands sign/zero extend to the Phase 29 `I32ToF32` / `U32ToF32` helpers, and float-to-integer casts use `F32ToI32` / `F32ToU32` before narrowing back to the requested integer width. The IR still rejects implicit mixed float/integer arithmetic. Dynamic float comparisons were deferred in Phase 28 and are lowered through `__rt_f32_cmp` in Phase 29.
