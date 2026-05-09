---
name: pic16cc context routing
alwaysApply: true
---

Open the right docs and files first for the task.

- CLI flags, warnings, artifact emission:
  - `src/cli/mod.rs`
  - `src/main.rs`
  - `src/lib.rs`
  - `tests/compiler_pipeline.rs`
- preprocessing, lexing, parsing:
  - `src/frontend/preprocessor.rs`
  - `src/frontend/lexer.rs`
  - `src/frontend/parser.rs`
  - `docs/frontend/overview.md`
- types, semantics, supported-subset rejection, diagnostics:
  - `src/frontend/semantic.rs`
  - `src/frontend/types.rs`
  - `src/diagnostics/*`
  - `docs/frontend/*.md`
  - `docs/diagnostics/overview.md`
- IR lowering or optimization:
  - `src/ir/lowering.rs`
  - `src/ir/model.rs`
  - `src/ir/passes.rs`
  - `docs/ir/*.md`
- PIC16 ABI, stack, banking, paging, helpers, ROM, function-pointer dispatch:
  - `src/backend/pic16/**`
  - `docs/backend/*.md`
  - `docs/runtime/*.md`
- device descriptors or public MCU headers:
  - `src/backend/pic16/devices.rs`
  - `include/pic16/*.h`
  - `docs/devices/overview.md`
  - `docs/developer-guide/adding-device.md`
- listing, map, HEX output:
  - `src/assembler/listing.rs`
  - `src/linker/map.rs`
  - `src/hex/intel_hex.rs`
- runtime execution behavior or emulator regressions:
  - `src/sim/mod.rs`
  - `tests/execution_sim.rs`
  - `docs/testing/phase19-emulator.md`
  - `docs/sim/pic16-core-emulator.md`

Prefer this order:

1. overview doc
2. specific phase doc
3. implementation files
4. relevant tests
