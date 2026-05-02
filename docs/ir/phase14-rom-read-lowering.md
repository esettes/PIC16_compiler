<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Phase 14 ROM Read Lowering

Phase 14 keeps ROM access explicit in IR.

## Model

- direct `rom_array[index]` and `__rom_read8()` lower to one typed ROM-read instruction for byte arrays
- direct `rom_array[index]` and `__rom_read16()` lower to one typed ROM-read instruction for 16-bit arrays
- ordinary RAM loads/stores are not reused for ROM because there is still no ROM pointer model

## Constant Index

- constant byte-index ROM reads may fold to inline constants before backend RETLW dispatch is needed
- constant 16-bit reads still preserve the documented little-endian element layout

## Dynamic Index

- dynamic index ROM reads stay as explicit ROM-read IR nodes
- backend then selects the RETLW-table call path instead of pretending ROM is RAM

## Restrictions

- no general ROM address arithmetic
- no ROM array decay
- no ROM read lowering inside ISR when the access would need dynamic dispatch
