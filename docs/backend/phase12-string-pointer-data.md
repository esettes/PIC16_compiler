<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Phase 12 String and Pointer Data

Phase 12 extends the Phase 10 RAM-backed static-data model to cover pointer-valued string literal initialization.

Storage model:

- string literals used as pointer initializers become anonymous static RAM byte arrays
- the backend allocates those arrays in ordinary data memory
- pointer-valued globals/statics may initialize from symbolic RAM addresses
- const storage is still RAM-backed in this phase

Startup behavior:

- string literal payload bytes are emitted through the same startup write path as other initialized static data
- pointer-valued globals/statics receive 16-bit RAM addresses during startup initialization
- listing output names both the string-literal object and the pointer object it initializes

Map/listing output:

- string literal symbols are grouped under a dedicated string-literal section in the rendered map
- listing comments show:
  - the literal payload initialization
  - the pointer object address initialization

Limits:

- no program-memory string/const placement
- no duplicate literal pooling pass
- no code-space pointer addressing model
<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
