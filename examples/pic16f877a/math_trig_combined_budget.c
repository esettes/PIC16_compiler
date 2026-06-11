#include <math.h>

/*
 * Phase 50 combined trig budget example.
 *
 * Suggested:
 * picc --target pic16f877a --math-profile balanced --size --memory-report \
 *      -I include -o build/math_trig_combined_budget.hex \
 *      examples/pic16f877a/math_trig_combined_budget.c
 *
 * Expected report:
 *   Trig runtime strategy: combined_sincos_tan_core
 *   __rt_f32_tan -> __rt_f32_sincos_core
 */

float angle;
float s;
float c;
float t;

void main(void) {
    angle = 0.7853982f;
    s = sinf(angle);
    c = cosf(angle);
    t = tanf(angle);
}
