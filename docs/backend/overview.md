# PIC16 `midrange14` Backend

Shared responsibilities:

- instruction selection
- banking
- paging
- startup
- lowering IR -> asm PIC16
- encoding 14-bit

Phase 3 additions:

- 16-bit load/store/cast lowering from Phase 2
- signed and unsigned relational compare lowering
- fixed helper-slot ABI for 16-bit args, returns, and pointers
- address materialization for symbols
- indirect scalar access through `FSR/INDF`
- element-size-aware indexing for 8-bit and 16-bit objects

Detail: [phase2-abi.md](/home/settes/cursus/PIC16_compiler/docs/backend/phase2-abi.md:1) and [phase3-memory-model.md](/home/settes/cursus/PIC16_compiler/docs/backend/phase3-memory-model.md:1)
