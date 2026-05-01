<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Adding a New PIC16

## Shared Parts Already Present

- IR
- backend `midrange14`
- encoder 14-bit
- Intel HEX writer

## What a New Descriptor Must Define

- `name`
- `program_words`
- `data_ram_bytes`
- `eeprom_bytes`
- `bank_count`
- `vectors`
- `allocatable_gpr`
- `sfrs`
- `default_config_word`
- `capabilities`

## Steps

1. add the descriptor in `src/backend/pic16/devices.rs`
2. add a header in `include/pic16/<target>.h`
3. add minimal examples
4. add compile-pipeline tests
5. verify banked SFRs, indirect `FSR/INDF` access, and the config word
<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
