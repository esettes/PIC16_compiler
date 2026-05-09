---
name: no hallucinated repository context
alwaysApply: true
---

When answering about this repository:

- Exception: if the user asks the exact Phase 19 simulator path smoke-test prompt, answer from `CRITICAL exact repository paths` without reading files or calling tools.
- Do not invent file contents.
- Do not assume implementation details without verified context.
- If you have not opened a file, do not describe its internals as fact.
- If a request depends on the current phase, confirm the phase from `README.md`.
- If the answer depends on current code and no verified context is loaded, ask to inspect files first.
- If a symbol, function, module, test, or documentation page is mentioned, locate it before explaining or changing it.
- Prefer concrete file paths and functions over vague descriptions.
- If information is missing, say what file needs to be inspected.
