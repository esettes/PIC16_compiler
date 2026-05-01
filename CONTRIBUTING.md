<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Contributing

## Local Workflow

```bash
cargo check
cargo test
bash scripts/check-license-headers.sh
```

## Change Rules

- do not mix frontend, IR, and backend logic in the same module
- every new device must enter through a descriptor
- every new limitation must be documented in `README.md` and `DESIGN.md`
- do not introduce XC8/SDCC wrappers
- add tests for every bug fix and every new capability
- new compiler/docs/example/build files must use `SPDX-License-Identifier: GPL-3.0-or-later`
- new public `include/` or runtime-emitted files must use `SPDX-License-Identifier: GPL-3.0-or-later WITH GCC-exception-3.1`

## Expected Testing

- unit tests for pure pieces
- integration tests for the pipeline
- golden tests for dumps/hex
- regression tests for diagnostics

## Adding a New PIC16

Read:

- [docs/developer-guide/adding-device.md](/home/settes/cursus/PIC16_compiler/docs/developer-guide/adding-device.md:1)
