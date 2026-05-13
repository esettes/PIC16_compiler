# Phase 28 Float Conversions

Phase 28 keeps `float` finite-only and adds runtime-backed casts where the existing Q16.16 bridge is safe.

Supported dynamic casts:

- `int` / `unsigned int` to `float`
- `float` to `int` / `unsigned int`
- `__fixed8_8` / `__ufixed8_8` to and from `float`
- `__fixed16_16` / `__ufixed16_16` to and from `float`

Rejected dynamic casts:

- `long` / `unsigned long` to `float`
- `float` to `long` / `unsigned long`

Those 32-bit integer conversions are rejected because Phase 28 does not provide a full integer-to-f32 conversion helper. Constants still fold when representable.

Dynamic float comparisons were deferred in Phase 28 and are implemented in Phase 29. Constant float comparisons fold at compile time.

ISR rule: any dynamic float cast is helper-backed and rejected inside interrupt handlers.
