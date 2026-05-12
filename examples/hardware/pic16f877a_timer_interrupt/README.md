<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# PIC16F877A Timer Interrupt Smoke

Minimal interrupt-vector smoke example. It increments `ticks` in the ISR and mirrors it on `PORTB`.

Commands:

```bash
make
make size
make flash FLASH_CMD="your-programmer-command"
```

This does not configure a complete timer peripheral model; adapt timer/SFR setup for your board before relying on timing.
