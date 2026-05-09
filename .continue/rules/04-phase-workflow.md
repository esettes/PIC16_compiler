---
name: pic16cc phase workflow
alwaysApply: true
---

Development is organized in phases.

Treat every feature or repair as phase-preserving work unless the user explicitly wants a new phase.

For any repair or feature:

1. Preserve previous behavior.
2. Confirm the construct is actually supported in the current phase before implementing it.
3. Keep unsupported cases as clear diagnostics.
4. Add or update the smallest adequate tests.
5. Update `README.md`, `DESIGN.md`, and relevant phase docs when behavior changes.
6. Add examples if the feature is user-facing.
7. Validate with:
   - `cargo check`
   - `cargo test`
   - `cargo clippy --all-targets -- -D warnings`
   - `cargo build --release` when CLI behavior changes.

Do not move to a new phase until the current one is stable.
