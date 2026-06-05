<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Phase 44 Trig Layout

Phase 44 adds helper-to-helper calls for trigonometry:

```text
__rt_f32_sin -> __rt_f32_sincos_core
__rt_f32_cos -> __rt_f32_sincos_core
```

Backend requirements:

- wrapper-to-core calls use page-safe `setpage` + `call`
- Phase 31/32 layout validation sees helper dependency edges
- stack reports include wrapper argument bytes plus shared-core call depth
- memory reports show wrapper/core variants and actual word counts
- ROM trig table placement remains separate and resource-fitted

Map/listing output groups all three helpers under math helpers. The ROM quarter-wave table remains under ROM table contribution:

```text
math helper __rt_f32_sin
math helper __rt_f32_cos
math helper __rt_f32_sincos_core
ROM RETLW table __rt_math_sin_qwave_table_balanced
```

No linker relaxation may remove required page setup across helper-to-helper calls unless the final layout proves it safe.

Phase 45 expands the shared core point set, so helper size can increase. Resource fitting and page validation still treat `__rt_f32_sincos_core` as one math helper section with page-safe wrapper calls.
