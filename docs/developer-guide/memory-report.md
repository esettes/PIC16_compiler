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
- non-empty program pages
- page setup relaxation count

## Detailed Report

`--memory-report` and `--memory-report-file` include:

- program range and vector/config addresses
- data RAM breakdown
- stack bounds and estimate
- function-pointer uncertainty
- allocatable/shared/reserved RAM ranges
- ROM table region
- page layout with used/free words per page
- program sections
- helper contribution with stack frame cost
- float helper contribution when `__rt_f32_*` helpers are emitted
- ROM float table contribution when `const __rom float[]` objects are emitted
- largest program-memory contributors

The report is deterministic so generated files can be diffed in CI.

## Map And Listing

When `--map` is enabled, the map begins with `Memory Summary`.

Phase 32 also adds a `Code Layout` section with per-page used/free words and section placement.

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

## Phase 27-30 Float Interaction

Float helpers are reported as `float helper`. Phase 28 cast helpers use the same category:

- `__rt_q16_16_to_f32`
- `__rt_f32_to_q16_16`
- `__rt_f32_cmp`
- `__rt_i32_to_f32`
- `__rt_u32_to_f32`
- `__rt_f32_to_i32`
- `__rt_f32_to_u32`

Generic helpers are large; prefer `--size` before hardware builds and expect resource fitting to reject programs that pull several float helpers on small targets.

Phase 30 ROM float tables are reported as `ROM RETLW table` contributions, not as helpers. A three-element table uses one entry word plus twelve `retlw` payload words. The map name includes tags such as `calibration [rom, const, 3 element(s), 12 byte(s)]`.
