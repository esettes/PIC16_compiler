<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Phase 21 Long Codegen

The PIC16 backend emits 32-bit values as four little-endian bytes. Inline byte-wise lowering covers:

- add/subtract with carry or borrow propagation
- bitwise `&`, `|`, `^`, and `~`
- equality and relational comparisons
- constant-count shifts
- loads, stores, casts, aggregate fields, unions, and arrays

Return ABI:

- byte 0 returns in `W`
- byte 1 returns in `__abi.return_high`
- byte 2 returns in `__abi.return_upper0`
- byte 3 returns in `__abi.return_upper1`

The Stack-first ABI is otherwise unchanged: 32-bit arguments occupy four caller-pushed bytes and 32-bit locals/temps occupy four frame bytes.

Carry is preserved around indirect frame-pointer setup during multi-byte shifts and rotates. Runtime helper calls save the low return byte across helper epilogue and caller stack cleanup.
