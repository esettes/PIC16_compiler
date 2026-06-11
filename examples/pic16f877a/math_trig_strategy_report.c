#include <math.h>

/*
 * Phase 50 strategy report example.
 *
 * Switch which assignment is active and inspect:
 *   Trig runtime strategy: sincos_shared_core
 *   Trig runtime strategy: isolated_tan_core
 *   Trig runtime strategy: combined_sincos_tan_core
 */

float angle;
float result_a;
float result_b;

void main(void) {
    angle = 1.5707963f;
    result_a = sinf(angle);
    result_b = cosf(angle);
}
