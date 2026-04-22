# General Architecture

`pic16cc` separates:

- C frontend
- internal IR
- shared PIC16 backend
- device layer
- final `.hex` emission

The target backend is the classic 14-bit PIC16 mid-range family. It is not a generic 8-bit CPU model.
