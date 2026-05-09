---
name: pic16cc supported subset and diagnostics
alwaysApply: true
---

This compiler intentionally supports a strict C subset.

Good agent behavior:

- confirm support in `README.md` and the relevant phase docs before implementing a feature
- if the construct is unsupported, add or preserve an explicit diagnostic
- do not silently widen support through partial lowering
- keep ownership clear:
  - frontend rejects invalid or unsupported source
  - IR makes supported behavior explicit
  - backend lowers only supported IR

Diagnostic expectations:

- point at user source with precise file/line/column when possible
- explain the unsupported construct directly
- prefer concrete messages over generic wording
- add regression coverage for accepted and rejected paths when support changes
