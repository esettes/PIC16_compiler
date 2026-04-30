# Phase 9: IR Switch Lowering

Phase 9 keeps the IR model simple. There is no dedicated switch instruction or terminator.

Chosen strategy:

- semantic analysis validates switch labels and controlling types
- IR lowering evaluates the controlling expression once
- the lowerer emits a linear chain of equality compare branches
- each `case` or `default` label becomes a normal CFG block
- `break` becomes a jump to one shared switch-end block, separate from any surrounding loop exit
- fallthrough becomes an ordinary jump into the next labeled block when no terminator intervenes

Approximate shape:

```text
tmp = switch_expr
if tmp == case0 goto case_0 else goto next_0
next_0:
if tmp == case1 goto case_1 else goto default_or_end
...
case_0:
  ...
  goto switch_end
case_1:
  ...
default:
  ...
switch_end:
```

Reasons no jump tables yet:

- PIC16 benefits less from table machinery on this limited subset
- compare chains are easier to inspect in IR, asm, listing, and tests
- existing backend compare-branch lowering already handles the required integer widths

Current limitation:

- case/default labels nested under unrelated control statements are rejected before lowering, so the IR lowerer only handles linear switch-body label flow
