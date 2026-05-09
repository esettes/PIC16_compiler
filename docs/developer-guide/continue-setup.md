<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Continue Setup For `pic16cc`

This repository now includes a project-specific Continue setup aimed at local agent work on the compiler.

Checked against Continue documentation on **May 9, 2026**:

- Continue local YAML config is user-level and lives at `~/.continue/config.yaml` on Linux/macOS.
- Repo-local rules in `.continue/rules/` are the supported way to inject project knowledge automatically.
- Continue CLI can use a specific config with `cn --config <path>`.

Because of that split, this repository provides:

- `.continue/config.yaml`: shared project config template for CLI use or for merging into your global Continue config
- `.continue/rules/*.md`: repo-local rules that teach the agent architecture, workflow, test selection, and diagnostics expectations
- `.continue/repo-context.md`: compact onboarding doc designed to be pinned as first context for local models
- `.continue/smoke-tests.md`: prompts that detect whether rules and context are actually active

## Recommended Local Models

The shared config expects these Ollama models:

```bash
ollama pull qwen2.5-coder:14b
ollama pull qwen2.5-coder:7b
ollama pull nomic-embed-text
```

Role mapping:

- `qwen2.5-coder:14b`: agent/chat/edit/apply
- `qwen2.5-coder:7b`: autocomplete
- `nomic-embed-text`: embeddings for indexing and retrieval

## How To Use In Continue CLI

Run Continue CLI against the repo config directly:

```bash
/home/settes/.npm-global/bin/cn --config /home/settes/cursus/PIC16_compiler/.continue/config.yaml
```

Inside the CLI session, open the repository root and work normally. The repo-local rules still apply because they live inside the workspace.

On this machine, `cn` is installed at `/home/settes/.npm-global/bin/cn`. Add this directory to `PATH` if you want to call it as plain `cn`.

Headless smoke test:

```bash
/home/settes/.npm-global/bin/cn --config /home/settes/cursus/PIC16_compiler/.continue/config.yaml --auto -p --silent "List the exact paths of files related to the Phase 19 simulator. Do not explain. Do not guess. Only return existing paths."
```

## How To Use In The IDE Extension

The IDE extension uses your user-level config file. The practical setup is:

1. Open `~/.continue/config.yaml`.
2. Merge the model, context, and prompt blocks from this repo's `.continue/config.yaml`.
3. Keep this repository opened as the workspace so `.continue/rules/` auto-apply.
4. Let Continue finish indexing before expecting deep retrieval quality.

If you prefer multiple configurations, keep this repo file as the source of truth and copy from it into your active Continue config when needed.

## What The Repo Rules Teach The Agent

- compiler architecture and layer boundaries
- current phase-driven workflow
- where to look first for frontend, IR, backend, CLI, device, or simulator tasks
- which test layer to choose
- how to handle unsupported constructs and diagnostics
- exact canonical paths for high-risk areas such as Phase 19 simulator validation

The key benefit is that small local models stop wasting context on blind exploration and start from the repo's real invariants.

## Verify Rules Are Loaded

Continue docs say local workspace rules live under `.continue/rules/`, are joined into the system message for Agent, Chat, and Edit, and can be viewed through the rules toolbar.

In the IDE:

1. Open the Continue rules toolbar.
2. Confirm `CRITICAL exact repository paths` is visible and appears before broader repo rules.
3. Run the prompts in `.continue/smoke-tests.md`.

The fastest smoke test is:

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

If the answer includes any path outside that four-line list, the active Continue assistant is not using this repository's rules or context correctly.

## Recommended First Pinned Context

For most sessions, pin these first:

- `.continue/repo-context.md`
- `README.md`
- `DESIGN.md`

Then pin one layer overview:

- frontend: `docs/frontend/overview.md`
- IR: `docs/ir/overview.md`
- backend: `docs/backend/overview.md`
- simulator/execution: `docs/testing/phase19-emulator.md`

## Included Prompts

The shared config defines four reusable prompts:

- `repo-onboard`
- `phase-implementation`
- `compiler-review`
- `diagnostic-design`

Use them to force a consistent workflow with local models, especially when the model is medium-sized and tends to skip repository inspection.

## Operational Advice For Agent Mode

- Ask for one compiler concern at a time: frontend, IR, backend, CLI, simulator, or docs.
- Mention the target test layer if you already know it.
- For bugfixes, include the failing example, diagnostic text, or wrong artifact shape.
- For new support, name the phase doc that should change if you know it.
- Prefer exact file references over broad prompts like "fix the compiler."

## Limits

- `.continue/config.yaml` is not guaranteed to be auto-loaded by the IDE extension as a workspace config; treat it as the checked-in project template.
- Repo-local rules are the most reliable project-scoped mechanism in current Continue docs.
- Embedding quality depends on `nomic-embed-text` being installed and indexing completing successfully.
