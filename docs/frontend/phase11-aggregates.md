# Phase 11 Aggregates

Phase 11 extends the frontend aggregate model without changing the packed-layout or data-space pointer assumptions from earlier phases.

## Supported

- arrays inside structs
- nested named struct fields
- nested aggregate initializer lists
- designated struct-field initializers: `.field = value`
- designated array-index initializers: `[index] = value`
- string literal initialization for `char` / `unsigned char` array fields
- whole-struct assignment between compatible complete struct types

## Layout Rules

- structs remain packed in declaration order
- array fields occupy contiguous bytes inside the containing struct
- nested struct fields contribute their full packed size at the field offset
- member access composes offsets through nested `.` / `->` chains

## Initializer Rules

- omitted aggregate leaves are zero-filled
- top-level omitted array size is inferred from:
  - brace initializer extent
  - string literal length including trailing null
- array designator indices must be constant, non-negative, and in range
- duplicate designated fields or array indices are rejected
- string literal initializers must fit including the trailing null byte

## Struct Assignment

- allowed only when source and destination are the same complete named struct type
- assignments to `const` aggregate objects are rejected
- incompatible named-struct assignment is rejected even when byte layout matches

## ISR Restrictions

- local aggregate initializers remain rejected inside interrupt handlers
- whole-struct assignment remains rejected inside interrupt handlers
- ordinary scalar nested-field reads/writes remain allowed when they stay inline-safe under existing ISR rules

## Current Limits

- no multidimensional arrays
- no chained designators such as `.outer.inner = 1`
- no anonymous nested struct/enum fields without declarators
- no pointers to incomplete struct types
