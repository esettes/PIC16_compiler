<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Phase 32 Page-Aware Layout

Phase 32 adds page-aware layout metadata and reporting. It does not add C language features.

## Section Model

The backend infers code sections from final labels after encoding:

- reset vector, interrupt vector, and startup
- user functions
- runtime helpers
- fixed-point helpers
- float helpers
- dynamic shift helpers
- function-pointer dispatchers
- ROM RETLW tables
- stack overflow trap and internal labels

Each section records:

- name
- kind
- start/end address
- word count
- start/end page

This model feeds `--memory-report` and `.map`.

## Page Reports

`--size` includes a compact page line:

```text
Page layout: page 0: 712 used, page 1: 143 used
```

`--memory-report` includes:

```text
Page layout
-----------
page 0: 0x0000..0x07FF used=... free=...
```

The map includes `Code Layout` with page usage and section placement.

Phase 33 extends the same reports with runtime helper category totals so page pressure can be traced back to integer, fixed, float, conversion, shift, dispatcher, or ROM-table costs.

## Placement Policy

Current Phase 32 placement is deterministic and preserves the existing emission order:

1. vectors
2. startup
3. user functions
4. function-pointer dispatchers
5. runtime helpers
6. stack trap
7. ROM tables

The compiler now reports this placement by page and keeps Phase 31 validation active. It does not yet reorder sections to pack pages.

## Failure Policy

If layout is unsafe after relaxation, compilation fails. The compiler does not emit an invalid HEX to hide the problem.
