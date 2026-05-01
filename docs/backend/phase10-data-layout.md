<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Phase 10: Backend Static Data Layout

Phase 10 keeps all supported static data in PIC16 RAM.

Storage model:

- globals, file-scope statics, and static locals occupy absolute RAM slots
- const objects use the same RAM allocation path as mutable objects
- there is still no separate program-memory const section or code-space string pool
- 16-bit scalar initializers store low byte first, then high byte

Startup behavior:

- zero-init objects emit startup clears for every byte in the object
- initialized scalar objects emit direct constant stores
- initialized array/struct objects emit one startup byte store per payload byte
- static locals are initialized once at startup, not every time their enclosing function runs

Static-data readability:

- startup assembly includes comments that mark `init` or `zero` actions for each static object
- map entries annotate user data with tags such as `[const]`, `[static]`, and `[static local]`
- listings inherit the startup comments because they render the final assembly stream

Why no ROM-style data placement yet:

- the current pointer model is data-space only
- code-space string pointers and const-address-space loads would require a new frontend/IR/backend contract
- byte-store startup code is simpler to verify against generated `.asm`, `.lst`, `.map`, and `.hex` artifacts on PIC16
<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
