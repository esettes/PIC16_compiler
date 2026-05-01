<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Phase 13 ROM Lowering

Phase 13 keeps the existing RAM-pointer IR model and adds one explicit ROM-read path.

Frontend result:

- explicit `const __rom` byte arrays are recorded as program-memory objects
- `__rom_read8(table, index)` becomes a dedicated typed ROM-read expression

IR shape:

- one ROM-read instruction carrying:
  - destination temp
  - ROM symbol id
  - index operand

Lowering rules:

- ROM objects do not become startup RAM payloads
- ROM reads do not become general pointer arithmetic
- no ROM pointer temps are introduced
- ordinary RAM strings for pointer initialization remain unchanged from Phase 12

Current limits:

- no direct ROM pointer values
- no ROM writes
- no ROM reads in ISR
- no wider-than-byte ROM element lowering
<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
