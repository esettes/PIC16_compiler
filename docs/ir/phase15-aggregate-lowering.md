<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Phase 15 Aggregate Lowering

Phase 15 keeps unions and bitfields inside the existing typed-IR model.

## Union Lowering

- union field access uses the ordinary aggregate base-address path plus constant offset `0`
- union initialization lowers to:
  - byte-wise zero-fill of the whole union storage
  - byte overlay for the selected field initializer
- whole-union assignment reuses the existing byte-wise aggregate-copy lowering already used for structs

## Bitfield Lowering

Bitfields do not add a dedicated backend-only shortcut.

Read:

1. load the containing byte or word
2. shift right by the field bit offset
3. mask to the field width

Write:

1. load the containing byte or word
2. clear the target bit range
3. mask and shift the assigned value
4. OR the shifted value into the cleared storage unit
5. store the updated byte or word

## Limits

- only unsigned `char`/`int`-sized bitfields are accepted
- address-of bitfields is rejected before IR generation
- no dedicated bitfield or union-copy IR opcode is introduced
- ISR rejection for whole-aggregate copy stays in semantic analysis
