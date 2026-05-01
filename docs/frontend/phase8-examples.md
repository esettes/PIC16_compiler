<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Phase 8 Examples

Example: struct and initializer

```c
struct Point { unsigned int x; unsigned int y; };
struct Point p = { 100, 200 };

unsigned int arr[4] = { 1, 2 }; // rest zero-filled
```

Example: enum

```c
enum Flags { A = 1, B, C = 8 };
enum Flags f = B;
```
<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
