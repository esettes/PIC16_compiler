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
- selected runtime profile
- selected math profile
- selected math accuracy policy
- used/available program words
- modeled data RAM usage and total device RAM
- static data bytes
- reserved software stack region
- estimated max stack
- ROM table words
- runtime helper count
- runtime helper words by category
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
- runtime helper contributors with actual words, estimated words, ABI args, local bytes, stack frame cost, category, required-by text, and target constraints
- runtime helper dependency graph
- selected math profile and math helper variant names
- selected math accuracy policy, including the Phase 40 precise `sqrtf` validated tolerance
- float helper contribution when `__rt_f32_*` helpers are emitted
- math helper contribution when `fabsf`, `truncf`, `floorf`, `ceilf`, `roundf`, or `sqrtf` pull finite math helpers
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

Float arithmetic/comparison helpers are reported as `float helper`; Phase 36 math helpers are reported as `math helper`. Phase 28 cast helpers use the float category:

- `__rt_q16_16_to_f32`
- `__rt_f32_to_q16_16`
- `__rt_f32_cmp`
- `__rt_i32_to_f32`
- `__rt_u32_to_f32`
- `__rt_f32_to_i32`
- `__rt_f32_to_u32`

Generic helpers are large; prefer `--size` before hardware builds and expect resource fitting to reject programs that pull several float helpers on small targets.

Phase 30 ROM float tables are reported as `ROM RETLW table` contributions, not as helpers. A three-element table uses one entry word plus twelve `retlw` payload words. The map name includes tags such as `calibration [rom, const, 3 element(s), 12 byte(s)]`.

## Phase 33 Runtime Cost Interaction

Phase 33 adds category totals:

```text
Runtime helpers: 3509 words
  integer: 0  division: 0  fixed: 0  float: 3509  math: 420  conversion: 0  shift: 0  dispatchers: 0
```

Use `--runtime-profile small` to select compact helper variants where implemented and to get earlier helper-budget warnings. `balanced` is the default. `fast` currently uses the same helper bodies as balanced.

Phase 38 adds math profile reporting:

```text
Math profile: compact
__rt_f32_sqrt: category=math variant=compact_approx ...
```

`compact` and default `balanced` currently use `variant=compact_approx` for dynamic `sqrtf`. `precise` uses `variant=precise_table_refined` and reports the Phase 40 validated tolerance for positive inputs in `[0.25, 64.0]`. The precise variant is larger than compact, so compare `Program words` and `math` helper totals before selecting it on small targets.

Phase 34/35 add variant/dependency detail for compact helpers:

```text
__rt_div_u32: category=division variant=small ... deps=__rt_u32_divmod_core
__rt_div_q16_16: category=fixed variant=small ... deps=__rt_div_uq16_16
__rt_f32_sub: category=float variant=small ... deps=__rt_f32_add
Runtime Helper Dependency Graph
__rt_div_u32 -> __rt_u32_divmod_core
```

This means `small` can reduce program words but may add helper-to-helper call depth. Check `--stack-report` when using compact helpers on small targets.
