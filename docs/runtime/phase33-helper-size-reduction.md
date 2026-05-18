<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Phase 33/34 Helper Size Reduction

Phase 33 focuses on cost visibility and safe pruning.

Implemented behavior:

- no duplicate helper bodies are emitted for one helper label
- unused helpers are not emitted
- constant-folded expressions do not retain obsolete helper requirements
- helper word cost is computed from final encoded labels, not only estimates
- helper costs are grouped by category in reports
- function-pointer dispatcher word cost is reported separately

Phase 34 adds the first real shared primitive path. With `--runtime-profile small`, 32-bit division and modulo wrappers share `__rt_u32_divmod_core` instead of emitting fully independent standalone bodies.

Representative `unsigned long` division plus modulo on `PIC16F877A`:

```text
balanced: Program words 3730, runtime helpers 2840
small:    Program words 3659, runtime helpers 2773
```

The saving is intentionally modest: correctness, page safety, and stack reporting are preserved. Shared helpers may add helper-to-helper calls, so inspect `--stack-report` when using `small`.

## Budget Warnings

Runtime profiles control warning thresholds:

- `small`: warns at helper code above 30% of program memory
- `balanced`: warns at helper code above 75%
- `fast`: currently same threshold and helper bodies as balanced

Program overflow remains a hard error regardless of profile.

## Target Guidance

On `PIC16F628A`, prefer integer or fixed-point code and inspect `--memory-report` before enabling float helpers.

On `PIC16F877A`, finite float comparison and conversion helpers are often practical, but full float multiply/divide helper sets may still be too large for real firmware with ROM tables and application code.
