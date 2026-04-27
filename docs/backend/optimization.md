# Phase 7 Backend Optimization

Phase 7 improves generated PIC16 quality without changing the language subset, ABI, ISR model, or runtime-helper contract.

## Goals

- reduce instruction count
- reduce unnecessary helper calls
- reduce frame pressure from dead temps
- minimize redundant bank/page updates
- keep `.map` / `.lst` outputs easier to read

## Pass Order

For `-O1`, `-O2`, and `-Os`, the pipeline is:

1. IR constant propagation and folding
2. IR dead code elimination
3. IR temp-slot compaction
4. backend lowering with helper fast paths
5. backend peephole cleanup

`-O0` skips the Phase 7 optimization passes.

## IR-Side Optimization

Current IR work:

- propagate known constants through copies, arithmetic inputs, calls, stores, indirect ops, and terminators
- fold constant casts, unary ops, and binary ops
- simplify constant branches into direct jumps
- clear unreachable blocks before codegen
- remove dead temp-producing instructions
- compact temp ids so frame-scoped temps occupy fewer slots

Effect:

- smaller CFG presented to the backend
- fewer frame bytes reserved for temps
- fewer useless loads/stores emitted later

## Helper Fast Paths

Before emitting a Phase 5 helper call, codegen now checks cheap inline cases.

Current fast paths:

- `x * 0 -> 0`
- `x * 1 -> x`
- `x * 2^n -> x << n`
- `0 / x -> 0`
- `x / 1 -> x`
- unsigned `x / 2^n -> x >> n`
- `0 % x -> 0`
- `x % 1 -> 0`
- unsigned `x % 2^n -> x & (2^n - 1)`
- constant-count shifts stay inline

These fast paths keep the same fixed-width semantics as the rest of the backend.

## Peephole Cleanup

After assembly emission, the backend applies conservative local rewrites.

Current patterns:

- `movf X,w` followed by `movwf X` -> drop the store
- duplicate adjacent `movwf X` -> keep one
- duplicate adjacent `bcf` / `bsf` on the same bit -> keep one
- duplicate adjacent `setpage label` -> keep one
- overwritten adjacent W loads (`movlw` / `clrw`) -> keep only the final load

These rules are intentionally local and conservative. They do not attempt global register allocation or control-flow restructuring.

## Banking and Paging

Phase 7 keeps the existing explicit PIC16 bank/page model and improves reuse.

Current behavior:

- `select_bank` only emits `RP0` / `RP1` changes for bits that actually changed
- writes to `STATUS` invalidate cached bank knowledge
- duplicate `setpage` pseudo-ops are removed by peephole cleanup

This improves size and reduces needless STATUS traffic without changing correctness rules.

## Reporting

`--opt-report` prints a compact summary after a successful compile.

Current counters:

- propagated and folded IR expressions
- simplified branches
- pruned unreachable blocks
- removed dead instructions
- removed temp slots
- removed peephole instructions by category
- helper calls avoided

## Limits

- no global data-flow framework yet
- no cross-basic-block register allocation
- no helper inlining
- no call reordering
- no semantics changes for ISR restrictions
- Stack-first ABI and ISR save/restore remain untouched
