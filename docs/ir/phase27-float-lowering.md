# Phase 27 Float IR Lowering

Float values lower as 32-bit scalar operands carrying raw f32 bits.

Lowering rules:

- float literals become raw-bit constants
- float copies, loads, stores, parameters, and returns reuse 32-bit ABI paths
- constant float arithmetic/comparisons fold before backend codegen
- dynamic float add/sub/mul/div lower as ordinary binary IR with `float` result type
- boolean float comparisons lower through the existing branch-to-value path when folded

The backend maps dynamic float arithmetic to runtime helpers or narrow inline fast paths. Mixed float/integer arithmetic is not implicitly inserted in IR during Phase 27.
