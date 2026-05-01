<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Phase 9: Frontend Switch Support

Phase 9 adds source-level `switch`, `case`, and `default` statements for the existing integer subset.

Supported switch expression types:

- `char`
- `unsigned char`
- `int`
- `unsigned int`
- `enum` declarations, using the compiler's fixed 16-bit `int` enum representation

Supported case labels:

- integer constant expressions
- enum constants
- values representable in the controlling switch type

Supported behavior:

- controlling expression is evaluated once
- one matching case transfers control into the switch body
- `default` runs when no case matches
- when no case matches and no `default` exists, the whole switch is skipped
- `break` exits only the innermost enclosing switch; in mixed loop/switch nesting it does not exit an outer loop
- fallthrough into the next adjacent case/default label is allowed when no `break`, `return`, or other control transfer intervenes
- nested switches work
- switches inside loops work
- loops inside switches work

Diagnostics:

- duplicate case values after normalization to the switch type
- multiple `default` labels in one switch
- `case` outside a switch
- `default` outside a switch
- non-constant case labels
- non-integer switch expressions
- case values not representable in the switch type

Current limitations:

- case/default labels must stay in the switch body flow or nested blocks
- labels nested under unrelated control statements like `if`, `while`, or `for` are rejected in phase 9
- no implicit-fallthrough warning is emitted in this phase
<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
