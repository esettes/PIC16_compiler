<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Phase 16 Aggregate Layout

Phase 16 extends the packed aggregate backend model to multidimensional RAM arrays.

## Layout

- multidimensional arrays are laid out row-major with no padding between adjacent elements
- struct fields containing multidimensional arrays use the same packed declaration-order layout as other fields
- union fields containing multidimensional arrays still begin at offset `0`

## Access

- constant-index accesses fold to base-plus-constant-offset lowering when possible
- dynamic index accesses reuse ordinary 8-bit/16-bit arithmetic and indirect memory operations
- no separate backend-only array helper or hidden runtime is introduced

## Restrictions

- multidimensional arrays remain RAM-only
- no multidimensional ROM table emission
- no pointer-decay ABI for multidimensional parameters
