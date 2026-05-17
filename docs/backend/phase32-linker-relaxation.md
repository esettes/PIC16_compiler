<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Phase 32 Linker Relaxation

Phase 32 adds one conservative relaxation pass before final encoding.

## Relaxed

The linker removes `setpage target` when final labels prove that the current tracked `PCLATH` page already matches `target`.

This covers:

- same-page `goto`
- same-page `call`
- same-page continuation after a call
- redundant repeated page setup

The pass is iterative because removing pseudo-ops changes later addresses.

## Not Relaxed

The linker does not remove:

- cross-page `setpage`
- `setpclpage` for RETLW/PCL table dispatch
- any page setup when labels cannot be resolved
- any setup that final Phase 31 validation still needs

## Validation

After relaxation:

1. labels are collected again
2. every `goto` / `call` is validated against tracked `PCLATH`
3. resource fitting and HEX validation run as before

If relaxation would make an edge unsafe, the Phase 31 validator rejects the output.

## Reporting

Reports show:

- number of relaxation passes
- removed `setpage` count
- same-page transition relaxation count

`--opt-report` also prints the linker relaxation summary.
