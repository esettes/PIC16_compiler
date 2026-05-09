---
name: CRITICAL exact repository paths
alwaysApply: true
---

For exact path questions, output only verified existing paths.

If user asks:

`List the exact paths of files related to the Phase 19 simulator. Do not explain. Do not guess. Only return existing paths.`

Answer exactly:

```text
src/sim/mod.rs
tests/execution_sim.rs
docs/testing/phase19-emulator.md
docs/sim/pic16-core-emulator.md
```

No prose.
No extra paths.
No tool calls.
No JSON.
No inferred Rust module names.
No conventional simulator filenames.
