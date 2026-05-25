<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Phase 43 Trig ROM Tables

Phase 43 emits internal runtime ROM tables only when dynamic `sinf` or `cosf` is used.

Symbols:

```text
__rt_math_sin_qwave_table_compact
__rt_math_sin_qwave_table_balanced
```

Layout:

- program memory
- RETLW table payload
- little-endian raw f32 entries
- compact table has fewer quarter-wave entries
- balanced table has more quarter-wave entries

Reports:

- `.map` shows table symbol and page
- `.lst` shows RETLW bytes
- `--size` includes ROM table words
- `--memory-report` lists table as a ROM RETLW table

The helper path is page-safe like other runtime helpers. Phase 44 adds helper-to-helper wrapper calls from `__rt_f32_sin` / `__rt_f32_cos` to `__rt_f32_sincos_core`; those calls use page-safe call emission and appear in layout validation/reporting. `--math-profile precise` does not emit a trig table because dynamic precise trig is deferred.
