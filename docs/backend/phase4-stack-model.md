# Phase 4 Stack Model

This document describes software stack shape used by Phase 4 backend.

Phase status:

- this stack model is the active baseline for Phase 6
- frozen in this branch for stabilization work

## Runtime State

Backend-managed helper slots:

- `stack_ptr.lo`
- `stack_ptr.hi`
- `frame_ptr.lo`
- `frame_ptr.hi`
- `return_high`
- `scratch0`
- `scratch1`

Startup initializes:

- `SP = stack_base`
- `FP = stack_base`

`stack_base`, `stack_end`, and helper locations appear in emitted map output.

## Growth Direction

Software stack grows upward.

- push byte: store at `SP`, then `SP = SP + 1`
- caller cleanup: `SP = SP - arg_bytes`

## Frame Layout

For callee with `A` argument bytes, `L` local bytes, `T` temp bytes:

- `FP + 0 .. A - 1`: arguments
- `FP + A`: saved caller `FP` low
- `FP + A + 1`: saved caller `FP` high
- `FP + A + 2 .. A + 1 + L`: locals and local arrays
- `FP + A + 2 + L .. A + 1 + L + T`: IR temps

Frame size reported by backend comments is:

- `frame_bytes = 2 + L + T`

It excludes argument bytes because caller owns them.

## Prologue

Current backend prologue:

1. copy caller `FP` into scratch
2. set `FP = SP - arg_bytes`
3. push saved caller `FP`
4. advance `SP` by `locals + temps`

Result:

- `FP` points at argument base
- `SP` points at top of frame storage

## Epilogue

Current backend epilogue:

1. load saved caller `FP` from `FP + arg_bytes`
2. set `SP = FP + arg_bytes`
3. restore caller `FP`
4. `RETURN`

This drops locals, temps, and saved `FP`, while leaving caller-owned argument bytes for caller cleanup.

## Access Strategy

Frame and pointer accesses use indirect PIC16 data memory instructions:

1. program `FSR`
2. derive `STATUS.IRP`
3. read or write via `INDF`

Direct banked file-register instructions remain in use for globals, SFRs, and helper slots.

## Safety Limits

- maximum stack depth is computed statically across non-recursive call graph
- oversized frames or call chains fail compilation
- recursion remains unsupported
- returning pointers that may refer to stack locals is rejected conservatively
