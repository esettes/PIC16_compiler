#include <math.h>

/*
 * Phase 49 pruning check.
 *
 * This file uses only constant-folded tanf calls. It should not emit
 * __rt_f32_tan, __rt_f32_tan_core, __rt_f32_sincos_core, or trig ROM tables.
 *
 * Suggested:
 * picc --target pic16f877a --math-profile balanced --size --memory-report \
 *      -I include -o build/math_tanf_pruning.hex \
 *      examples/pic16f877a/math_tanf_pruning.c
 */

float folded_zero = tanf(0.0f);
float folded_one = tanf(0.78539816f);

void main(void) {
}
