<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Phase 25 Resource Fitting

Phase 25 adds target-aware resource validation for `pic16f628a` and `pic16f877a`.

The backend now derives a resource model after final encoding, when all user code, startup, runtime helpers, function-pointer dispatchers, stack traps, and ROM RETLW tables have concrete addresses.

## Device Descriptor Model

Each built-in target descriptor records:

- normal program-memory word range
- reset vector
- interrupt vector
- config word address
- allocatable GPR ranges
- shared GPR ranges
- reserved/SFR RAM ranges
- default stack candidate range
- ROM table placement range

The current allocator still uses the modeled direct GPR window plus shared ISR context window. The report also prints the datasheet-level total RAM byte count so users can see the difference between device capacity and currently modeled backend capacity.

## Program Memory Fit

Validation rejects generated words outside the target program range, except the config word emitted by the HEX writer. ROM tables are still allocated from high program memory downward and must not cross the Phase 14 RETLW page limit or overlap generated code.

Diagnostics include the target name, available address range, and highest offending word when available.

## RAM Fit

The report accounts for:

- static/global data
- file-scope statics and static locals
- RAM-backed string literals
- ABI/runtime slots
- ISR context slots
- reserved software stack region

Allocator failures now surface as data-RAM overflow diagnostics instead of silent layout failures.

## Stack Fit

Phase 25 reuses Phase 18 stack analysis:

- stack base/limit/capacity
- static max stack
- per-function frame sizes
- helper extra pressure
- ISR frame/context cost
- function-pointer target-set uncertainty

If the static max stack cannot fit in the reserved stack region, compilation fails.

## Reports

`--size` prints a compact summary. `--memory-report` prints deterministic detailed output. `--memory-report-file <path>` writes the same detailed report to disk.

The `.map` file starts with a compact memory summary, and the `.lst` file starts with comment-form resource summary lines.

Phase 32 extends these reports with page/layout data:

- `--size` includes non-empty program pages and page-setup relaxation counts
- `--memory-report` includes a `Page layout` block with used/free words per page
- `.map` includes a `Code Layout` block grouping sections by page
- section rows show page ranges for functions, helpers, dispatchers, ROM tables, and internal code

## Limitations

- Banked RAM allocation is still conservative and does not yet exploit every GPR bank on larger devices.
- Page-boundary diagnostics focus on currently modeled RETLW-table constraints.
- Peripheral/SFR reservation is descriptor-driven and intentionally minimal.
