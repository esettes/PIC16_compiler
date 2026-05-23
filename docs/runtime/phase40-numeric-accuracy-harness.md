<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Phase 40 Numeric Accuracy Harness

Phase 40 adds simulator-backed numeric checks for finite math helpers.

The harness pattern is:

1. compile a C fixture with `picc`
2. run the HEX with `pic16-sim`
3. read raw f32 result bits from RAM through the generated `.map`
4. decode the bits with host Rust `f32`
5. compare against host-side expected values with a documented tolerance

Supported comparisons:

- raw f32 bit equality for exact policy values
- absolute error checks for approximate numeric validation
- compact-vs-precise error comparisons

For `sqrtf`, Phase 40 uses host `f32::sqrt` as the reference for positive finite test inputs. Negative finite inputs are policy values and must return raw `0.0f`.

The harness is intentionally small. It is not a proof of IEEE compliance; it is a repeatable runtime accuracy check for generated PIC16 code.
