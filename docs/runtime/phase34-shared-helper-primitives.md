<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Phase 34 Shared Helper Primitives

Phase 34 adds the first real shared runtime primitive:

```text
__rt_u32_divmod_core
```

It is used only by `--runtime-profile small`.

## Covered Helpers

The compact profile rewrites these helpers as wrappers:

- `__rt_div_u32`
- `__rt_mod_u32`
- `__rt_div_i32`
- `__rt_mod_i32`

The wrappers preserve existing behavior:

- unsigned division returns quotient
- unsigned modulo returns remainder
- signed division/modulo normalize signs and restore quotient/remainder sign
- dynamic division by zero returns `0`
- constant division by zero remains a diagnostic

## ABI

`__rt_u32_divmod_core` follows the Stack-first ABI:

- dividend: 4 bytes
- divisor: 4 bytes
- mode byte: `0` for quotient, `1` for remainder
- return: 32-bit value through `W`, `return_high`, `return_upper0`, `return_upper1`

The helper is cataloged as a division helper and appears in `.map`, `.lst`, `--size`, and `--memory-report`.

## Page Safety

Wrapper-to-core calls are emitted as normal page-safe helper calls. Phase 31/32 validation still checks final control-flow edges after layout and relaxation.

## Limits

Phase 34 does not yet compact Q16.16, Q8.8, or float helper internals. Those helpers remain candidates for later size work after separate simulator coverage.
