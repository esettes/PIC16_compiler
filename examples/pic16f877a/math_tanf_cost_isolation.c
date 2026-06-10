#include <math.h>

/*
 * Phase 49 tan-only cost check.
 *
 * Suggested:
 * picc --target pic16f877a --math-profile balanced --size --memory-report \
 *      -I include -o build/math_tanf_cost_isolation.hex \
 *      examples/pic16f877a/math_tanf_cost_isolation.c
 *
 * Expected report shape:
 *   __rt_f32_tan -> __rt_f32_tan_core
 *   no __rt_f32_sin, __rt_f32_cos, or __rt_f32_sincos_core
 */

float angle;
float tangent;

void main(void) {
    angle = 0.7853982f;
    tangent = tanf(angle);
}
