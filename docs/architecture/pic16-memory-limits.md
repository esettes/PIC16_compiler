<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# PIC16 Memory Limits

Phase 25 makes device limits explicit for the built-in targets.

## PIC16F628A

- program memory: `0x0000..0x07FF` words, 2048 words total
- reset vector: `0x0000`
- interrupt vector: `0x0004`
- config word: `0x2007`
- modeled allocatable GPR: `0x0020..0x006F`
- modeled shared GPR: `0x0070..0x007F`
- ROM table region: `0x0005..0x07FF`

## PIC16F877A

- program memory: `0x0000..0x1FFF` words, 8192 words total
- reset vector: `0x0000`
- interrupt vector: `0x0004`
- config word: `0x2007`
- modeled allocatable GPR: `0x0020..0x006F`
- modeled shared GPR: `0x0070..0x007F`
- ROM table region: `0x0005..0x1FFF`

## Layout Policy

Program memory contains vector stubs, startup, functions, dispatchers, runtime helpers, optional stack trap, and ROM RETLW tables. ROM tables are placed from high memory downward.

Data memory contains ABI helper slots, globals/statics/string literals, optional ISR context slots, and the software stack region.

The compiler emits errors when generated program words exceed the target range, when data allocation cannot fit in modeled RAM, or when the static stack estimate exceeds the reserved stack region.

## Limitation

The current backend models a conservative GPR subset. Reports show both modeled capacity and device total RAM so future allocator expansion can be measured without changing CLI output shape.
