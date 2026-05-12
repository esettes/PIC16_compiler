<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# PIC16F628A LED Blink

Hardware smoke example for a LED on `PORTB0`.

Commands:

```bash
make
make size
make flash FLASH_CMD="your-programmer-command"
```

`FLASH_CMD` is a placeholder. Install/configure your PICkit-style tool separately.

Expected behavior: `RB0` toggles after programming. Add resistor/LED wiring appropriate for your board.
