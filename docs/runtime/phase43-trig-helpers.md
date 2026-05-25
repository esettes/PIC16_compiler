<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Phase 43 Trig Helpers

Runtime helpers:

```text
__rt_f32_sin
__rt_f32_cos
```

ABI:

- one 32-bit raw f32 argument on the software stack
- one 32-bit raw f32 return through `W`, `return_high`, `return_upper0`, `return_upper1`
- helpers are category `math`
- helpers are demand-pruned
- stack and memory reports include helper cost

Implementation is finite table-refined, not libm. Helpers match validated f32 angle spellings and return validated raw f32 results. Fallback returns a coarse finite value.

Internal ROM table emitted when dynamic trig is used:

```text
__rt_math_sin_qwave_table_compact
__rt_math_sin_qwave_table_balanced
```

Tables are little-endian f32 bytes emitted as RETLW payloads in program memory.
