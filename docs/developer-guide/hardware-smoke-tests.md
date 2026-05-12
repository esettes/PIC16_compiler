<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Hardware Smoke Tests

Phase 26 adds hardware-oriented examples:

- `examples/hardware/pic16f628a_led_blink/`
- `examples/hardware/pic16f877a_led_blink/`
- `examples/hardware/pic16f877a_timer_interrupt/`

Each directory includes:

- `main.c`
- `Makefile`
- `README.md`
- config bits
- compile command
- configurable flash command
- wiring/behavior note

Use:

```bash
make
make size
make flash FLASH_CMD="your-programmer-command"
```

`make sim` is a bounded core-only smoke run. It does not emulate peripherals or real board timing.

These examples are hardware workflow fixtures, not guaranteed board support packages.
