---
name: Rust compiler implementation style
globs: ["src/**/*.rs", "tests/**/*.rs"]
---

For Rust code in this repository:

- Keep code modular and maintainable.
- Prefer explicit types and clear data structures.
- Do not hide compiler behavior behind vague helpers.
- Keep frontend, IR, backend, linker, and simulator responsibilities separated.
- Prefer small, direct helpers over broad abstractions.
- Preserve `#![forbid(unsafe_code)]` assumptions.
- Respect `Cargo.toml` lint policy; new code should be clippy-clean.
- Add tests for semantic behavior, diagnostics, codegen shape, and execution when relevant.
- Do not add broad refactors unless necessary.
- Comments and documentation inside code must be in English.
- Public or important functions should have a brief English doc comment.
- Preserve SPDX headers.
