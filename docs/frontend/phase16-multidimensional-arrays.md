<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Phase 16 Multidimensional Arrays

Phase 16 adds fixed-size multidimensional RAM arrays and chained designators.

## Supported

- declarations such as `unsigned char matrix[2][3];`
- repeated indexing like `matrix[i][j]`
- nested initializer lists with zero-fill
- multidimensional array fields inside complete structs and unions
- chained designators such as `.a.x`, `[1][2]`, and `.glyph[1][2]`

## Layout

- layout is row-major
- every inner dimension must be explicit
- one optional outermost dimension may be inferred from a brace initializer
- struct and union field access composes the containing aggregate offset with the row-major element offset

## Restrictions

- multidimensional arrays do not decay to data pointers
- multidimensional array parameter types remain rejected
- multidimensional `__rom` arrays remain rejected
- helper-requiring dynamic multidimensional indexing remains rejected inside ISR
