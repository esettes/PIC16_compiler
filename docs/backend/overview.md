# PIC16 `midrange14` Backend

Shared responsibilities:

- instruction selection
- banking
- paging
- startup
- lowering IR -> asm PIC16
- encoding 14-bit

Phase 2 additions:

- 16-bit load/store/cast lowering
- signed and unsigned relational compare lowering
- fixed helper-slot ABI for 16-bit args and returns

Detail: [phase2-abi.md](/home/settes/cursus/PIC16_compiler/docs/backend/phase2-abi.md:1)
