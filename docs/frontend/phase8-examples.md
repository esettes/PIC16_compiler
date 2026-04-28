# Phase 8 Examples

Example: struct and initializer

```c
struct Point { unsigned short x; unsigned short y; };
struct Point p = { 100, 200 };

unsigned short arr[4] = { 1, 2 }; // rest zero-filled
```

Example: enum

```c
enum Flags { A = 1, B, C = 8 };
enum Flags f = B;
```
