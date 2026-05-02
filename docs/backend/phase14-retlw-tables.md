<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Phase 14 RETLW Tables

Phase 14 emits explicit `__rom` arrays as RETLW-backed program-memory tables.

## Layout

- every ROM object gets one program-memory symbol
- backend emits one entry instruction (`addwf PCL, f`) plus one `retlw k` per payload byte
- 16-bit elements use little-endian byte order
- ROM data is kept separate from reset vector `0x0000`, interrupt vector `0x0004`, ordinary code, and config word `0x2007`

## Access

- constant-index reads may inline one direct constant result
- dynamic reads call the generated RETLW table entry
- map and listing output show ROM symbols in a dedicated ROM section

## Restrictions

- one Phase 14/17 ROM table payload must fit one 255-byte RETLW page
- ROM pointers are not introduced
- multidimensional ROM arrays are not emitted in this phase
