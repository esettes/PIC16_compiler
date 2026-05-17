<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Phase 33 Runtime Helper Catalog

Phase 33 centralizes runtime helper metadata in `RuntimeHelper`.

Each helper records:

- label
- category
- ABI argument bytes
- local bytes
- stack frame bytes
- required-by text
- estimated words
- dependency list
- page-sensitive flag
- target constraints

Categories are:

- integer
- division
- fixed
- float
- conversion
- shift

The catalog feeds codegen, stack estimates, memory reports, map grouping, listing comments, and diagnostics.

## Dependency Graph

Helpers currently keep independent bodies, so most dependency lists are empty. The graph is still validated before helper emission so future shared helper primitives cannot introduce unknown or cyclic dependencies silently.

## Pruning

Helpers are emitted only when codegen marks a concrete runtime use. Declarations and folded constants do not pull helper bodies.

Examples that should stay helper-free:

```c
float f = 1.5f;
long l = 3L;
__fixed16_16 q = 1.0q16_16;
```

## Reports

`--memory-report` includes:

```text
Runtime Helper Contributors
Runtime Helper Dependency Graph
```

`.lst` helper bodies include comments such as:

```text
; runtime helper: float helper=__rt_f32_cmp required_by=finite f32 arithmetic/comparison
```
