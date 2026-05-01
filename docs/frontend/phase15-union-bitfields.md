<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Phase 15 Union and Bitfields

Phase 15 adds conservative named `union` support plus basic unsigned bitfields.

## Supported

- named `union` declarations
- global and local union objects
- pointers to complete named unions
- union fields of supported scalar, pointer, array, struct, or union type
- `.` and `->` access for union members
- first-field union initializer: `union U x = {1};`
- designated union initializer: `union U x = {.word = 1000};`
- whole-union assignment between compatible complete union types
- unsigned bitfields on `unsigned char` and `unsigned int`
- bitfield reads and writes in ordinary expressions and assignments

## Union Layout

- every union field has byte offset `0`
- union storage size is the maximum field size
- current packed aggregate model inserts no extra padding
- nested named union fields inside structs use the same packed byte offsets as other fields

## Bitfield Layout

- only `unsigned char` and `unsigned int` bitfields are accepted
- width must be a positive integer constant expression
- width must fit inside the declared storage unit
- packing is LSB-first within the active storage unit
- bitfields do not cross their declared storage-unit boundary
- ordinary non-bitfield fields start after the current bitfield storage unit

## Initialization

- union initialization zero-fills the whole union storage first
- one positional element initializes the first declared field
- one designated `.field = value` initializer selects that field explicitly
- extra initializer elements, duplicate designators, and unknown fields are diagnosed

## Restrictions

- anonymous nested union fields are still rejected
- signed bitfields are still rejected
- unnamed bitfields are still rejected
- taking the address of a bitfield is rejected
- ROM unions and ROM bitfield objects are unsupported
- local aggregate initialization and whole-aggregate copy remain rejected inside ISR code
