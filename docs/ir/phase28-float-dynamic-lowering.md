# Phase 28 Float Dynamic Lowering

Phase 28 adds explicit cast IR for the finite float bridge:

- `Q16ToF32`
- `F32ToQ16`

Frontend lowering routes supported dynamic casts through Q16.16 or UQ16.16 first, then emits one of these cast nodes.

Examples:

```c
int i;
float f;

f = (float)i;
i = (int)f;
```

Lowering shape:

```text
i -> Q16.16 -> Q16ToF32
F32ToQ16 -> Q16.16 -> i
```

The IR still rejects implicit mixed float/integer arithmetic. Dynamic float comparisons were deferred in Phase 28 and are lowered through `__rt_f32_cmp` in Phase 29.
