# PIC16 `midrange14` Backend

Shared backend responsibilities:

- instruction selection
- direct-bank selection
- indirect-bank selection
- paging
- startup
- IR -> PIC16 asm lowering
- 14-bit word encoding

Current backend phase: **Phase 5 arithmetic helpers on Phase 4 Stack-first ABI**

Backend owns:

- software stack helper slots: `stack_ptr`, `frame_ptr`
- return helper slot: `return_high`
- short-lived scratch slots: `scratch0`, `scratch1`
- caller-pushed stack argument lowering
- per-call frame lowering for locals and IR temps
- `FSR/INDF` indirect access for pointers and frame storage
- Phase 5 runtime helper emission for multiply/divide/modulo and dynamic shifts

Current call contract:

- caller pushes args left-to-right
- callee saves caller `FP`
- callee sets `FP` to callee arg base
- callee allocates locals + temps above saved `FP`
- callee restores `SP` to caller arg top
- callee restores caller `FP`
- caller subtracts argument bytes after return

Current return contract:

- 8-bit: `W`
- 16-bit: `W` low + `return_high` high
- pointer: same as 16-bit integer

Phase 5 helper contract:

- helpers use same stack-first ABI as normal functions
- caller pushes helper args left-to-right, low byte first inside each scalar
- caller cleans helper arg bytes after return
- helpers may mutate their own arg slots as working storage
- helper locals/count/flags live above saved `FP` in helper frame storage
- helper labels are emitted only when used and appear in `.map` / `.lst`

Current backend docs:

- [phase4-stack-first-abi.md](/home/settes/cursus/PIC16_compiler/docs/backend/phase4-stack-first-abi.md:1)
- [phase4-stack-model.md](/home/settes/cursus/PIC16_compiler/docs/backend/phase4-stack-model.md:1)
- [phase5-helper-calling.md](/home/settes/cursus/PIC16_compiler/docs/backend/phase5-helper-calling.md:1)
- [../runtime/phase5-arithmetic-helpers.md](/home/settes/cursus/PIC16_compiler/docs/runtime/phase5-arithmetic-helpers.md:1)
- [../ir/phase5-arithmetic-lowering.md](/home/settes/cursus/PIC16_compiler/docs/ir/phase5-arithmetic-lowering.md:1)
- [../ir/phase4-call-lowering.md](/home/settes/cursus/PIC16_compiler/docs/ir/phase4-call-lowering.md:1)

Historical docs:

- [phase2-abi.md](/home/settes/cursus/PIC16_compiler/docs/backend/phase2-abi.md:1)
- [phase3-memory-model.md](/home/settes/cursus/PIC16_compiler/docs/backend/phase3-memory-model.md:1)
