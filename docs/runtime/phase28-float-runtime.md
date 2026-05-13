# Phase 28 Float Runtime

New helper labels:

- `__rt_q16_16_to_f32`
- `__rt_f32_to_q16_16`

Both helpers use Stack-first ABI:

- argument width: 4 bytes
- return width: 4 bytes
- return slots: `W`, `return_high`, `return_upper0`, `return_upper1`
- helper frame: reported in stack and memory reports

Policy:

- finite values only
- no NaN/Inf guarantee
- no `double`
- no math library
- dynamic 32-bit integer float casts are provided by Phase 29 helpers, not this Q16.16 bridge
- no helper use inside ISR

The conversion bridge is intentionally Q16.16 based. It is useful for sensor scaling and fixed-point interop, not a complete IEEE conversion library.
