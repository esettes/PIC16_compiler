# Phase 30 ROM Float Tables

ROM float tables use the existing RETLW object layout.

Layout:

- one entry instruction: `addwf PCL,f`
- one `retlw` per payload byte
- element width: 4 bytes
- byte order: little-endian IEEE f32 raw bits

Example:

```c
const __rom float table[] = { 1.0f };
```

Raw f32 bits:

```text
0x3F800000
```

RETLW payload bytes:

```text
00 00 80 3F
```

Resource behavior:

- each float element costs four RETLW payload words
- each table also costs one entry word
- the Phase 14/25 single-table payload limit remains 255 bytes
- map, listing, `--size`, and `--memory-report` show ROM table contribution
- final HEX validation still checks vectors, config word, program range, and overlap

ISR behavior:

- constant-index reads are allowed only when lowered inline
- dynamic-index reads are rejected because they call the RETLW table dispatch path

Known limitation: helper-backed float arithmetic/comparison remains large. For most firmware, copy ROM-read values into RAM globals/statics before helper-heavy expressions so resource and simulator behavior stays predictable.
