# Phase 2 IR Lowering

## Goal

Phase 2 keeps the AST/backend split intact while making 16-bit integer codegen real.

## Semantic Output

Before IR lowering, semantic analysis now:

- infers integer literal width/sign more precisely
- inserts explicit casts for integer coercions
- rejects unsupported equal-width mixed signedness compares
- keeps comparison expressions typed as boolean-like `unsigned char`

## IR Extensions

Phase 2 adds:

- `IrInstr::Cast`
- typed `IrCondition::NonZero`
- typed `IrCondition::Compare`

These changes let the backend distinguish:

- operand width
- operand signedness
- compare-vs-value behavior

without inspecting AST nodes.

## Boolean Materialization

When comparison result is needed as a value, lowering uses:

1. branch form for the compare
2. true block writes `1`
3. false block writes `0`
4. join block continues with normalized boolean temp

This is reused for:

- `!expr`
- `lhs < rhs` assigned into a variable
- loop and conditional expressions

## Compare Lowering

Direct branch conditions keep compare operands in `IrCondition::Compare`:

- equality / inequality
- signed relations
- unsigned relations

The condition carries the operand type so backend codegen can pick the correct PIC16 expansion.

## Cast Lowering

The backend receives explicit cast intent:

- `ZeroExtend`
- `SignExtend`
- `Truncate`
- `Bitcast`

That keeps integer width transitions local and testable.

## Optimization Impact

Constant folding can now:

- fold 16-bit arithmetic
- fold 16-bit casts
- resolve compare operands in branch conditions

Dead code elimination now performs backward liveness over SSA-like temps so dead temp chains disappear instead of only the last unused temp.
