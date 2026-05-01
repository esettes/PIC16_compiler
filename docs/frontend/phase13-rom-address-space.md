<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Phase 13 ROM Address Space

Phase 13 adds one explicit program-memory object syntax:

```c
const __rom unsigned char table[] = {1, 2, 3};
const __rom char msg[] = "OK";
```

Supported:

- file-scope ROM objects only
- `char` / `unsigned char` arrays only
- brace-list or string-literal initializers
- omitted array size inference from those initializers
- ROM reads through `__rom_read8(table, index)`

Rules:

- `__rom` objects must also be `const`
- plain `const` still means RAM-backed const storage
- ROM arrays do not decay to data-space pointers
- direct `table[index]` on ROM objects is rejected
- ROM pointer types are not modeled yet
- ROM reads inside ISR are rejected in this phase

Diagnostics:

- non-const ROM object
- local ROM object
- unsupported ROM element/object type
- ROM/data-pointer mixing
- direct ROM indexing
- unsupported ROM pointer form
<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
