<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# pic16cc

An experimental C compiler written in Rust for classic mid-range PIC16 microcontrollers. It compiles C code directly into a programmable Intel HEX file without requiring an external assembler.

Supported devices:

- `PIC16F628A`
- `PIC16F877A`

The project provides two command-line tools:

- `picc`: compiles C code and generates firmware.
- `pic16-sim`: runs generated HEX files in a CPU simulator for testing and debugging.

> **Status:** Experimental. Use it for learning, research, and carefully validated firmware. It is not yet a replacement for a production C toolchain.

## Workflow

```text
C source ──> picc ──> firmware.hex ──> external programmer ──> PIC16
                  ├─> firmware.map
                  ├─> firmware.lst
                  └─> pic16-sim (hardware-free testing)
```

## Quick start

Requirements: Linux, Rust 1.85 or later, and Cargo. `make` is only needed for examples that provide a Makefile.

Install both executables from the repository root:

```bash
cargo install --path .
picc --version
pic16-sim --version
```

To build without installing, run `cargo build --release` and use the executables under `target/release/`.

### 1. Compile an example

```bash
picc \
  --target pic16f877a \
  -I include -O2 -Wall -Wextra -Werror \
  --map --list-file --size \
  -o build/arithmetic.hex \
  examples/sim/arithmetic_sim.c
```

Generated files:

| File | Purpose |
| --- | --- |
| `build/arithmetic.hex` | Firmware to program or simulate |
| `build/arithmetic.map` | Symbols and memory layout |
| `build/arithmetic.lst` | Generated instruction listing |

### 2. Run it in the simulator

```bash
pic16-sim \
  --target pic16f877a \
  build/arithmetic.hex \
  --map build/arithmetic.map \
  --run-until __halt \
  --print-symbol result
```

Expected output:

```text
result = 5 (0x05)
```

### 3. Compile your own firmware

Replace the target, input, and output paths for your project:

```bash
picc --target pic16f628a -I include -O2 -Wall -Wextra \
  --map --list-file --verify-hex \
  -o build/main.hex src/main.c
```

List supported targets and available options:

```bash
picc --list-targets
picc --help
pic16-sim --help
```

### 4. Program a microcontroller

`picc` generates the HEX file but does not include a USB driver for PICkit or other programmers. Configure an external programming tool compatible with your hardware.

Build and flash a hardware example:

```bash
make -C examples/hardware/pic16f628a_led_blink
make -C examples/hardware/pic16f628a_led_blink flash \
  FLASH_CMD="your-programmer-command"
```

## Main features

- Native end-to-end pipeline: preprocessing, C frontend, optimization, machine code, and Intel HEX generation.
- 8-, 16-, and 32-bit integers, fixed-point types, and finite `float` support.
- Structs, unions, arrays, data pointers, ROM tables, interrupts, and constrained function pointers.
- Size, memory, and stack reports; MAP and LST files; final HEX validation.
- Configurable runtime and math profiles for balancing code size and behavior.

## Important limitations

- Implements a C subset rather than full ISO C compatibility.
- No `double`, recursion, general ROM/code pointer model, or complete math library.
- `float` and `math.h` support is finite and deliberately constrained; helper-heavy programs may exceed PIC resources.
- The simulator focuses on the CPU and generated firmware. It does not fully model peripherals, timing, or real hardware.

## Examples and documentation

- [PIC16F628A examples](examples/pic16f628a/)
- [PIC16F877A examples](examples/pic16f877a/)
- [Hardware examples](examples/hardware/)
- [Simulator workflow](docs/developer-guide/simulator-workflow.md)
- [Device programming](docs/developer-guide/programming-pic16.md)
- [Compiler architecture](docs/architecture/overview.md)
- [Supported math subset](docs/developer-guide/math-subset.md)
- [Detailed project status](PROJECT_STATUS.md)
- [Contributing guide](CONTRIBUTING.md)
- [Changelog](docs/CHANGELOG.md)

## License

Source code, tests, and documentation are licensed under [GPL-3.0-or-later](COPYING). Public headers and runtime material intended for compiled firmware use [GPL-3.0-or-later with the GCC Runtime Library Exception 3.1](COPYING.RUNTIME).
