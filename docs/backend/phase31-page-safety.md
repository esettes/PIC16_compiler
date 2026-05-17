<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Phase 31 Page Safety

Phase 31 hardens generated PIC16 control flow against stale `PCLATH`.

PIC16 `goto` and `call` instructions carry only low target bits. The high program-page bits come from `PCLATH<4:3>`. A raw jump that is correct in one layout can fail when helpers, ROM tables, or dispatchers grow enough to move the target to another page.

## Emission Rules

- Unconditional jumps emit a page setup for the final target before `goto`.
- Calls emit a page setup for the callee before `call`.
- Conditional branches use centralized skip-safe sequences that set `PCLATH` for the fallthrough label before the local skip path and set it again for the taken target before `goto`.
- Runtime helper loops, function-pointer dispatchers, stack-check traps, ROM-read paths, and switch/compare chains use the same backend branch helpers.
- Raw local `goto` / `call` is valid only when final layout validation can prove the current tracked page matches the target page.

## Layout Validation

After final addresses are known, the encoder walks assembly in address order and tracks the current control page.

It validates:

- every `goto` target page matches tracked `PCLATH`
- every `call` target page matches tracked `PCLATH`
- `setpage` / `setpclpage` pseudo-ops point at known labels
- labels reset the validator's entry-page model to the label address page
- unsafe edges report the source symbol, from/target addresses, from/target pages, and tracked page

This catches helper-internal loops and dispatch edges that become unsafe only after final code layout.

## Artifacts

`.map` code symbols include page metadata:

```text
0810  __rt_div_uq16_16  page=1
```

`.lst` shows page-control comments:

```text
; setpage __rt_f32_cmp ; page-safe control-flow target
```

The simulator remains unchanged. It already models `goto` / `call` through `PCLATH`, so page bugs appear as real runtime failures.

## Remaining Limits

Phase 31 validates emitted edges; it does not add a linker relaxation pass or move code to optimize page placement. Oversized programs still fail through Phase 25/26 resource and HEX validation.
