<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Programming PIC16 Devices

Phase 26 keeps `picc` as a compiler. It does not implement USB PICkit protocols.

## Compile

```bash
picc --target pic16f628a -I include --verify-hex -o build/blink.hex examples/pic16f628a/blink.c
```

`--verify-hex` prints reset vector, interrupt vector, config word, checksum, EOF, and target-range validation.

## Size And Memory

```bash
picc --target pic16f628a -I include --size --memory-report -o build/blink.hex examples/pic16f628a/blink.c
```

## Print Programmer Command

```bash
picc --print-program-command --target pic16f628a -o build/blink.hex
picc --print-program-command --program-cmd "your-programmer-command" --target pic16f628a -o build/blink.hex
```

The compiler only prints or runs an external command that you provide. It does not assume MPLAB IPE, `pk3cmd`, `mdb`, or any other tool exists.

## Flash From Make

```bash
make flash FLASH_CMD="your-programmer-command"
```

All example Makefiles use:

```make
FLASH_CMD ?= echo "Configure FLASH_CMD to program"
FLASH_ARGS ?=
flash: $(OUT)
	$(FLASH_CMD) $(FLASH_ARGS) $(OUT)
```

## Optional `--program`

`--program` runs `--program-cmd <cmd>` after successful compile:

```bash
picc --target pic16f628a -I include --program --program-cmd "your-programmer-command" -o build/blink.hex blink.c
```

If `--program` is used without `--program-cmd`, compilation fails with a clear diagnostic.

## Limitations

- No built-in hardware programmer driver.
- No vendor tool path is hardcoded.
- Tests do not require hardware.
