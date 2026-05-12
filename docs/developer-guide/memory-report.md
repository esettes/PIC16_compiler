<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Memory Report

Phase 25 adds three compiler options:

```bash
picc --target pic16f877a -I include --size -o build/app.hex app.c
picc --target pic16f877a -I include --memory-report -o build/app.hex app.c
picc --target pic16f877a -I include --memory-report-file build/app.mem -o build/app.hex app.c
```

## Compact Size Output

`--size` prints:

- target
- used/available program words
- modeled data RAM usage and total device RAM
- static data bytes
- reserved software stack region
- estimated max stack
- ROM table words
- runtime helper count

## Detailed Report

`--memory-report` and `--memory-report-file` include:

- program range and vector/config addresses
- data RAM breakdown
- stack bounds and estimate
- function-pointer uncertainty
- allocatable/shared/reserved RAM ranges
- ROM table region
- program sections
- helper contribution with stack frame cost
- float helper contribution when `__rt_f32_*` helpers are emitted
- largest program-memory contributors

The report is deterministic so generated files can be diffed in CI.

## Map And Listing

When `--map` is enabled, the map begins with `Memory Summary`.

When `--list-file` is enabled, the listing begins with comment-form resource summary lines before the word dump.

## Diagnostics

Phase 25 adds clearer failures for:

- program memory overflow
- data RAM overflow
- stack region overflow
- ROM table/code overlap
- config word overlap
- missing descriptor ranges

Existing ROM page-size and ISR/helper diagnostics remain unchanged.

## Phase 26 Interaction

Phase 26 makes final HEX validation always-on. `--size` and `--memory-report` remain detailed reporting surfaces, but invalid program-memory output is now rejected even when reports are not requested.

Use `--verify-hex` with `--memory-report` when preparing a hardware build:

```bash
picc --target pic16f628a -I include --size --memory-report --verify-hex -o build/app.hex app.c
```

## Phase 27 Float Interaction

Float helpers are reported as `float helper`. Generic helpers are large; prefer `--size` before hardware builds and expect resource fitting to reject programs that pull several float helpers on small targets.
