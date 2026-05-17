<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Phase 33 Helper Size Reduction

Phase 33 focuses on cost visibility and safe pruning.

Implemented behavior:

- no duplicate helper bodies are emitted for one helper label
- unused helpers are not emitted
- constant-folded expressions do not retain obsolete helper requirements
- helper word cost is computed from final encoded labels, not only estimates
- helper costs are grouped by category in reports
- function-pointer dispatcher word cost is reported separately

The compiler does not yet split helpers into shared primitive subroutines. That is intentionally deferred because sharing may reduce program words but increase call overhead, page pressure, and stack usage.

## Budget Warnings

Runtime profiles control warning thresholds:

- `small`: warns at helper code above 30% of program memory
- `balanced`: warns at helper code above 75%
- `fast`: currently same threshold and helper bodies as balanced

Program overflow remains a hard error regardless of profile.

## Target Guidance

On `PIC16F628A`, prefer integer or fixed-point code and inspect `--memory-report` before enabling float helpers.

On `PIC16F877A`, finite float comparison and conversion helpers are often practical, but full float multiply/divide helper sets may still be too large for real firmware with ROM tables and application code.
