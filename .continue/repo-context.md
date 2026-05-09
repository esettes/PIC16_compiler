<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# pic16cc Repo Context

Use this file as the first pinned context item for Continue agent work in this repository.

## Exact Answer: Phase 19 Simulator Paths

For this prompt:

```text
List the exact paths of files related to the Phase 19 simulator. Do not explain. Do not guess. Only return existing paths.
```

Return exactly:

```text
src/sim/mod.rs
tests/execution_sim.rs
docs/testing/phase19-emulator.md
docs/sim/pic16-core-emulator.md
```

## Identity

- Project: `pic16cc`
- End-user CLI binary: `picc`
- Language/runtime: Rust 2024, `#![forbid(unsafe_code)]`
- Domain: native C-subset compiler for classic 14-bit PIC16 mid-range MCUs
- Supported devices today: `PIC16F628A`, `PIC16F877A`
- Current milestone: Phase 19 execution validation on top of the Phase 4 stack-first ABI and later frontend/backend phases

## Primary Sources Of Truth

- `README.md`: current supported subset, user-visible behavior, phase status
- `DESIGN.md`: architecture, invariants, ABI, backend policy
- `docs/architecture/overview.md`: compact layered architecture
- `docs/frontend/overview.md`: accepted syntax/types/subset boundaries
- `docs/ir/overview.md`: typed IR responsibilities and lowering rules
- `docs/backend/overview.md`: PIC16 ABI, helpers, banking, paging, stack, ROM, dispatcher rules
- `docs/developer-guide/testing.md`: test strategy and command matrix
- `docs/diagnostics/overview.md`: diagnostic output shape

## Pipeline Map

1. CLI/orchestration
   - `src/main.rs`
   - `src/lib.rs`
   - `src/cli/mod.rs`
2. Source loading + preprocessing
   - `src/common/source.rs`
   - `src/frontend/preprocessor.rs`
3. Lexing/parsing/semantic analysis
   - `src/frontend/lexer.rs`
   - `src/frontend/parser.rs`
   - `src/frontend/semantic.rs`
   - `src/frontend/types.rs`
   - `src/frontend/ast.rs`
4. Typed IR lowering and optimization
   - `src/ir/lowering.rs`
   - `src/ir/model.rs`
   - `src/ir/passes.rs`
5. PIC16 backend
   - `src/backend/pic16/devices.rs`
   - `src/backend/pic16/midrange14/codegen.rs`
   - `src/backend/pic16/midrange14/runtime.rs`
   - `src/backend/pic16/midrange14/encoder.rs`
   - `src/backend/pic16/midrange14/asm.rs`
6. Output artifacts
   - `src/assembler/listing.rs`
   - `src/linker/map.rs`
   - `src/hex/intel_hex.rs`
7. Execution validation
   - `src/sim/mod.rs`
   - `tests/execution_sim.rs`

## Phase 19 Simulator Paths

Only these repository paths are related to the current Phase 19 simulator/execution-validation implementation:

- `src/sim/mod.rs`
- `tests/execution_sim.rs`
- `docs/testing/phase19-emulator.md`
- `docs/sim/pic16-core-emulator.md`

For exact path answers, return only paths from this list or paths verified with `rg --files`.

## Architectural Invariants

- Frontend decides language legality and supported subset boundaries.
- IR makes calls, memory, comparisons, and aggregate behavior explicit.
- Backend owns PIC16-specific lowering, software stack, banking, paging, helpers, ROM tables, and encoding.
- Backend must not inspect AST directly.
- Frontend must not fake backend support for unsupported constructs.
- Unsupported source constructs must fail with explicit diagnostics.
- Simulator validates generated HEX after compilation; it is not an alternate compiler path.

## Current High-Risk Areas

- Phase 4 stack-first ABI correctness
- `FSR/INDF` frame and pointer access
- PIC16 bank/page selection (`STATUS`, `PCLATH`)
- helper-aware lowering for `*`, `/`, `%`, `<<`, `>>`
- ISR restrictions and context save/restore
- ROM/data-space separation
- function-pointer dispatch-ID lowering
- stack-report and stack-check behavior

## Where To Look First By Task

- CLI flags, warning behavior, output artifacts:
  - `src/cli/mod.rs`
  - `src/lib.rs`
  - `tests/compiler_pipeline.rs`
- parse/type/semantic rejection:
  - `src/frontend/parser.rs`
  - `src/frontend/semantic.rs`
  - `src/frontend/types.rs`
  - `docs/frontend/*.md`
- IR shape or optimization:
  - `src/ir/lowering.rs`
  - `src/ir/model.rs`
  - `src/ir/passes.rs`
  - `docs/ir/*.md`
- PIC16 codegen, ABI, helpers, banking, paging, ROM:
  - `src/backend/pic16/**`
  - `docs/backend/*.md`
  - `docs/runtime/*.md`
- device descriptors or headers:
  - `src/backend/pic16/devices.rs`
  - `include/pic16/*.h`
  - `docs/devices/overview.md`
- map/listing/HEX issues:
  - `src/assembler/listing.rs`
  - `src/linker/map.rs`
  - `src/hex/intel_hex.rs`
- runtime behavior or emulator regressions:
  - `src/sim/mod.rs`
  - `tests/execution_sim.rs`
  - `docs/testing/phase19-emulator.md`
  - `docs/sim/pic16-core-emulator.md`

## Testing Matrix

- Rust unit tests:
  - isolated helper/module logic
- `tests/compiler_pipeline.rs`:
  - diagnostics
  - CLI behavior
  - `.asm`, `.ir`, `.map`, `.lst`, `.hex` shape
- `tests/execution_sim.rs`:
  - arithmetic correctness
  - stack/call behavior
  - pointer and aggregate runtime behavior
  - function-pointer dispatch
  - ROM reads

Core commands:

```bash
cargo check
cargo test
cargo test --test execution_sim
cargo clippy --all-targets -- -D warnings
```

Use `cargo build --release` when CLI behavior or release artifacts change.

## Change Expectations

When behavior changes:

- update `README.md` for user-visible subset or CLI behavior
- update `DESIGN.md` for architecture/invariant changes
- update relevant phase docs under `docs/`
- update tests
- update examples when the feature is user-facing

## Recommended Pinned Context For Agent Sessions

Minimal pin set:

- `.continue/repo-context.md`
- `README.md`
- `DESIGN.md`

Then add one overview doc by layer:

- frontend work: `docs/frontend/overview.md`
- IR work: `docs/ir/overview.md`
- backend work: `docs/backend/overview.md`
- simulator work: `docs/testing/phase19-emulator.md`
