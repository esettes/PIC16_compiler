<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Phase 3 to Phase 4 ABI Migration

Phase 4 replaces Phase 3 call/storage model.

## What Changed

Phase 3:

- fixed helper-slot call ABI
- `arg0` / `arg1`
- locals in static RAM
- temps in static RAM
- no software stack
- no 3+ argument calls

Phase 4:

- stack-first caller-pushed ABI
- no functional `arg0` / `arg1` path
- real software stack in backend
- per-call locals
- per-call IR temps
- 3+ argument calls supported

## New Call Contract

- caller pushes args
- callee saves caller `FP`
- callee sets `FP` to callee argument base
- callee allocates locals + temps
- callee restores `SP` to caller argument top
- callee restores caller `FP`
- caller subtracts argument bytes

## Why Migration Matters

Benefits:

- nested calls no longer depend on fixed argument slots
- call depth can include mixed signatures
- locals and temps are per invocation
- address-of local can be passed across calls inside one activation safely

Still constrained:

- recursion rejected
- stack depth is compile-time only
- pointer escape checks are conservative, not full alias analysis

## Source-level Compatibility Notes

Programs that fit old Phase 2/3 subset should still compile, but these behaviors changed:

- functions may take more than two arguments
- locals and temps now consume software stack capacity
- returning stack-local pointers now rejects more obvious alias chains
- emitted map files now include ABI helper and stack markers

## Documentation Map

Use these as current references:

- [../backend/phase4-stack-first-abi.md](/home/settes/cursus/PIC16_compiler/docs/backend/phase4-stack-first-abi.md:1)
- [../backend/phase4-stack-model.md](/home/settes/cursus/PIC16_compiler/docs/backend/phase4-stack-model.md:1)
- [../ir/phase4-call-lowering.md](/home/settes/cursus/PIC16_compiler/docs/ir/phase4-call-lowering.md:1)

Use these as historical references only:

- [../backend/phase2-abi.md](/home/settes/cursus/PIC16_compiler/docs/backend/phase2-abi.md:1)
- [../backend/phase3-memory-model.md](/home/settes/cursus/PIC16_compiler/docs/backend/phase3-memory-model.md:1)
<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
