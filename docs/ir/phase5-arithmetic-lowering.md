# Phase 5 Arithmetic Lowering

Phase 5 keeps Phase 4 call lowering and adds full typed lowering for:

- `*`
- `/`
- `%`
- `<<`
- `>>`

## Semantic Rules

Allowed operands:

- integer scalar types only

Rejected:

- pointer multiply/divide/modulo
- pointer shifts
- unsupported mixed signedness without explicit cast when both operands have equal width

Diagnostics:

- division by constant zero
- modulo by constant zero
- constant shift count `>=` bit width
- negative constant shift count

`-Wextra`:

- signed right shift warns that Phase 5 uses arithmetic shift semantics

## Integer Promotion Subset

Compiler does not implement full ISO C usual arithmetic conversions yet.

Current subset for `*`, `/`, `%`, `&`, `|`, `^`:

- identical types stay unchanged
- integer literal adopts other operand type when possible
- otherwise wider width wins
- equal-width mixed signedness is rejected unless cast is explicit

Current subset for shifts:

- result type is left operand type
- right operand is coerced to left operand type

## IR Shape

No special helper IR instruction was introduced.

IR still uses:

- `IrInstr::Binary { op, lhs, rhs, .. }`

Backend decides whether one binary op lowers:

- inline
- through a Phase 5 runtime helper call

This keeps helper selection backend-specific while preserving typed IR.

## Constant Folding

IR constant folding now evaluates:

- multiply
- divide
- modulo
- left shift
- right shift

using fixed-width PIC16-compatible integer semantics.

## Backend Selection

Backend chooses:

- inline identity/constant paths when cheap
- runtime helper when operation is expensive or dynamic

Examples:

- `a << 3` -> inline
- `a >> n` -> runtime helper
- `a * b` -> runtime helper unless one side matches cheap identity/power-of-two pattern

## Runtime Semantics

Documented current runtime behavior:

- dynamic divide/modulo by zero returns `0`
- dynamic shift counts clamp to bit width
- signed right shift is arithmetic
- arithmetic wraps/truncates to destination width
