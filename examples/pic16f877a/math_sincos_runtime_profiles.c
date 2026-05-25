// SPDX-License-Identifier: GPL-3.0-or-later
// Runtime profile small still uses shared trig core; math profile selects table/tolerance.
// picc --target pic16f877a --runtime-profile small --math-profile balanced --size --memory-report -I include -o build/sincos_small.hex examples/pic16f877a/math_sincos_runtime_profiles.c

#include <math.h>

float input_angle;
float output_sin;
float output_cos;

void main(void) {
    input_angle = 3.1415927f;
    output_sin = sinf(input_angle);
    output_cos = cosf(input_angle);
}
