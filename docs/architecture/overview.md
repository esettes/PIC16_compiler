# General Architecture

`pic16cc` separates:

- C frontend
- internal IR
- shared PIC16 backend
- device layer
- final `.hex` emission

The target backend is the classic 14-bit PIC16 mid-range family. It is not a generic 8-bit CPU model.

Phase 3 keeps that split intact while extending:

- 16-bit integer lowering from Phase 2
- lvalue/rvalue-aware semantic analysis
- typed IR address, indirect-load, and indirect-store nodes
- constrained data pointers and one-dimensional arrays
- PIC16 `FSR/INDF` indirect access without backend duplication per device
