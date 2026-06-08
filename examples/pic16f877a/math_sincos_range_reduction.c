// SPDX-License-Identifier: GPL-3.0-or-later
// Phase 46 hardens common multiples through +/-8pi with deterministic aliases.

#include <math.h>

float sin_three_pi;
float cos_three_pi;
float sin_four_pi;
float cos_four_pi;
float sin_eight_pi;
float cos_eight_pi;

void main(void) {
    float angle;

    angle = 9.424778f;
    sin_three_pi = sinf(angle);
    cos_three_pi = cosf(angle);

    angle = 12.566371f;
    sin_four_pi = sinf(angle);
    cos_four_pi = cosf(angle);

    angle = 25.132742f;
    sin_eight_pi = sinf(angle);
    cos_eight_pi = cosf(angle);
}
