---
name: pic16cc testing matrix
alwaysApply: true
---

Choose the smallest test layer that can actually prove the change.

- Rust unit tests:
  - isolated module logic
- `tests/compiler_pipeline.rs`:
  - diagnostics
  - CLI behavior
  - AST/IR/ASM/map/listing/HEX shape
- `tests/execution_sim.rs`:
  - arithmetic correctness
  - stack/call behavior
  - pointer dereference
  - aggregate layout/access behavior
  - function-pointer dispatch
  - ROM reads

Execution tests are preferred when output shape alone cannot prove correctness.

Core validation commands:

- `cargo check`
- `cargo test`
- `cargo test --test execution_sim`
- `cargo clippy --all-targets -- -D warnings`

Also run `cargo build --release` when user-visible CLI behavior or release artifacts change.
