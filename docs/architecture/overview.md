# General Architecture

`pic16cc` separates:

- C frontend
- internal IR
- shared PIC16 backend
- device descriptor layer
- final `.hex` emission

Target backend is classic 14-bit PIC16 mid-range family, not a generic 8-bit CPU model.

Current Phase 4 keeps that split intact while extending:

- stack-first caller-pushed ABI
- per-call frame storage for locals and IR temps
- typed IR call lowering for arbitrary argument counts
- explicit pointer and frame access through `FSR/INDF`
- PIC16 banking/paging without backend duplication per device

See:

- [../../DESIGN.md](/home/settes/cursus/PIC16_compiler/DESIGN.md:1)
- [../backend/overview.md](/home/settes/cursus/PIC16_compiler/docs/backend/overview.md:1)
- [../ir/overview.md](/home/settes/cursus/PIC16_compiler/docs/ir/overview.md:1)
