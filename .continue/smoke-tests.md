<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Continue Smoke Tests

Use these prompts after changing Continue setup or switching assistants.

## Rules Loaded

Prompt:

```text
List the exact paths of files related to the Phase 19 simulator. Do not explain. Do not guess. Only return existing paths.
```

Expected answer:

```text
src/sim/mod.rs
tests/execution_sim.rs
docs/testing/phase19-emulator.md
docs/sim/pic16-core-emulator.md
```

Failure signs:

- returns any path outside the expected answer
- returns prose instead of paths

Likely cause:

- workspace rules from `.continue/rules/` are not loaded
- the wrong Continue assistant/config is active
- the repository root is not the active workspace

## Context Retrieval

Prompt:

```text
Which file should I read first before changing PIC16 codegen? Answer with one path only.
```

Expected answer:

```text
docs/backend/overview.md
```

## No Hallucinated Source

Prompt:

```text
Does this repo have a Phase 19 simulator file outside the paths listed in .continue/repo-context.md? Answer yes or no only.
```

Expected answer:

```text
no
```
