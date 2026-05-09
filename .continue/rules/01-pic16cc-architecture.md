---
name: pic16cc architecture
alwaysApply: true
---

You are working inside the `pic16cc` repository.

`pic16cc` is a native Rust compiler for classic 14-bit PIC16 mid-range MCUs. The installed CLI executable is `picc`.

Before proposing or changing code:

- inspect the exact implementation files touched by the task
- inspect the matching overview or phase docs under `docs/`

Exception: if the user asks the exact Phase 19 simulator path smoke-test prompt, answer from `CRITICAL exact repository paths` without reading files or calling tools.

Primary sources of truth:

- `README.md`: current supported subset, current phase, user-visible behavior
- `DESIGN.md`: architecture, invariants, ABI, backend policy
- `docs/architecture/overview.md`: high-level layering summary
- `docs/frontend/overview.md`: accepted syntax/types and frontend limits
- `docs/ir/overview.md`: IR responsibilities and lowering rules
- `docs/backend/overview.md`: PIC16 codegen, stack, banking, paging, helpers, ROM
- `docs/developer-guide/testing.md`: test strategy

Core architecture:

- native compiler pipeline, not an XC8/SDCC/LLVM wrapper
- strict frontend -> IR -> backend layering
- stack-first ABI
- shared PIC16 `midrange14` backend
- Intel HEX, map, and listing emission
- internal emulator for execution validation of generated HEX

Hard invariants:

- frontend never owns PIC16 encoding details
- backend never inspects AST directly
- every unsupported construct must fail with a clear diagnostic
- do not add fake or partial support just to satisfy a test
- preserve stack-first ABI, bank/page correctness, and device-descriptor ownership

When behavior changes:

- update tests first or alongside code
- update `README.md` for user-visible changes
- update `DESIGN.md` for architecture/invariant changes
- update the relevant phase docs
- update examples when the feature is user-facing
