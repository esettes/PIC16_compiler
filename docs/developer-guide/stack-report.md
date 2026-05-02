<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Stack Report

Phase 18 adds two stack-visibility CLI surfaces:

- `--stack-report`
- `--stack-report-file <path>`

## Commands

Print report to stdout:

```bash
picc \
  --target pic16f877a \
  -I include \
  -O2 -Wall -Wextra \
  --stack-report \
  -o build/app.hex \
  app.c
```

Write report to file:

```bash
picc \
  --target pic16f877a \
  -I include \
  -O2 -Wall -Wextra \
  --stack-report-file build/app.stack \
  -o build/app.hex \
  app.c
```

Enable runtime checks too:

```bash
picc \
  --target pic16f877a \
  -I include \
  -O2 -Wall -Wextra \
  --stack-check \
  --stack-report \
  -o build/app.hex \
  app.c
```

## Report Shape

Report shows:

- target
- stack growth direction
- stack base / limit / capacity
- static max usage
- runtime stack-check status
- ISR frame and saved-context sizes
- function-pointer group counts
- per-function frame/helper/depth data

Typical per-function line:

```text
main: frame=6 args=0 locals=4 temps=0 helper_extra=0 max_stack=14 max_depth=3
```

Possible follow-up lines:

```text
  direct callees: init, work
  indirect fnptr<void(void)> -> off, on
```

## Reading Limits

- `max_stack` is conservative worst-case stack bytes while that function is active
- `max_depth` is call depth, not byte count
- `helper_extra` is extra helper arg/frame pressure caused by helper-requiring arithmetic
- unknown indirect target sets are reported explicitly
- recursion is still rejected, so reports describe bounded acyclic programs only

## Related Runtime Check

`--stack-check` is separate from reporting.

It emits inline overflow guards and a generated `__stack_overflow_trap` loop. Report text still works with checks on or off.
