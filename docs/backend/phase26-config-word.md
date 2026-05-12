<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Phase 26 Config Word

Phase 26 makes config words explicit and user-controllable.

## Model

Each target descriptor includes:

- config address
- valid mask
- default word
- reserved-bit policy
- known symbolic fields and values

Supported targets:

- `pic16f628a`: default `0x3F30`, config address `0x2007`
- `pic16f877a`: default `0x3F32`, config address `0x2007`

Reserved policy: preserve default bits for symbolic pragmas; reject raw config words that set bits outside the valid 14-bit mask.

## Syntax

Symbolic:

```c
#pragma config FOSC = INTRC_NOCLKOUT
#pragma config WDTE = OFF
```

Raw fallback:

```c
__config(0x3F18);
```

Do not mix raw and symbolic settings. Duplicate fields are rejected.

## Diagnostics

The compiler rejects:

- unknown config field
- unknown config value
- duplicate config setting
- raw config outside valid mask
- descriptor/default mismatch

The final config word is emitted in HEX and shown in map/listing resource summaries.
