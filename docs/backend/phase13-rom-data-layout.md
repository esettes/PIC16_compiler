# Phase 13 ROM Data Layout

Phase 13 uses a PIC16-friendly callable table representation for explicit ROM byte arrays.

Representation:

- each ROM object gets its own program-memory label
- backend emits:
  - one entry word: `addwf PCL,f`
  - one `retlw k` word per byte

Implications:

- one byte consumes one program word
- each table must fit inside one 256-word page
- current limit is 255 bytes per ROM object
- ROM symbols appear in a dedicated map section

`__rom_read8(table, index)` lowering:

- compare index against table length
- out-of-range read returns `0`
- in-range read loads index into `W`
- backend calls the ROM table label directly
- table returns the requested byte in `W` via `retlw`

Safety:

- ROM tables are allocated above generated code
- reset vector `0x0000` and interrupt vector `0x0004` remain untouched
- config word at `0x2007` remains outside normal program-word allocation

Current limits:

- no general ROM pointer/address model
- no ROM structs
- no ROM reads inside ISR
