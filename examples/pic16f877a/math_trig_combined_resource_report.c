#include <math.h>

/*
 * Phase 49 combined trig report check.
 *
 * Suggested:
 * picc --target pic16f877a --math-profile balanced --size --memory-report \
 *      -I include -o build/math_trig_combined_resource_report.hex \
 *      examples/pic16f877a/math_trig_combined_resource_report.c
 *
 * Expected report shape:
 *   __rt_f32_sin -> __rt_f32_sincos_core
 *   __rt_f32_cos -> __rt_f32_sincos_core
 *   __rt_f32_tan -> __rt_f32_tan_core
 */

float angle;
float sine_value;
float cosine_value;
float tangent_value;

void main(void) {
    angle = 0.7853982f;
    sine_value = sinf(angle);
    cosine_value = cosf(angle);
    tangent_value = tanf(angle);
}
