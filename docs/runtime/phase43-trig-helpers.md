<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Phase 43 Trig Helpers

Runtime helpers:

```text
__rt_f32_sin
__rt_f32_cos
__rt_f32_tan
__rt_f32_sincos_core
```

ABI:

- one 32-bit raw f32 argument on the software stack
- one 32-bit raw f32 return through `W`, `return_high`, `return_upper0`, `return_upper1`
- helpers are category `math`
- helpers are demand-pruned
- stack and memory reports include helper cost
- Phase 44 makes `__rt_f32_sin` and `__rt_f32_cos` wrappers around one shared core
- Phase 49 routes `__rt_f32_tan` to an isolated `__rt_f32_tan_core`

Implementation is finite table-refined, not libm. The shared sine/cosine core matches validated f32 angle spellings and returns validated raw f32 results. Phase 45 adds `pi/6`, `pi/3`, `2pi/3`, `5pi/6`, negative mirrors, and common `±3pi` / `±4pi` aliases. Phase 46 extends scoped common-multiple aliases through `±8pi`. Phase 47 stores those aliases with sign-normalized matching to reduce duplicated branches. Phase 49 keeps tangent values in `__rt_f32_tan_core`; odd `pi/2` pole-like points return finite `±32767.0f` saturation. Fallback returns a coarse finite value.

Internal ROM table emitted when dynamic trig is used:

```text
__rt_math_sin_qwave_table_compact
__rt_math_sin_qwave_table_balanced
```

Tables are little-endian f32 bytes emitted as RETLW payloads in program memory.

Pruning:

- unused `#include <math.h>` emits no trig helper or table
- dynamic `sinf` emits `__rt_f32_sin`, `__rt_f32_sincos_core`, and the selected table
- dynamic `cosf` emits `__rt_f32_cos`, `__rt_f32_sincos_core`, and the selected table
- dynamic `tanf` emits `__rt_f32_tan` and the Phase 49 isolated `__rt_f32_tan_core`
- using `sinf` and `cosf` emits only the requested wrappers, one shared sine/cosine core, and one table
- using `tanf` emits the tangent wrapper and `__rt_f32_tan_core`
