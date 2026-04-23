# General Architecture

`pic16cc` separates:

- C frontend
- internal IR
- shared PIC16 backend
- device layer
- final `.hex` emission

The target backend is the classic 14-bit PIC16 mid-range family. It is not a generic 8-bit CPU model.

Phase 2 keeps that split intact while extending:

- 16-bit integer lowering
- typed IR casts and compare nodes
- signed/unsigned relational lowering
- fixed-slot ABI support for 16-bit args and returns
