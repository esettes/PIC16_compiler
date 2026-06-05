// SPDX-License-Identifier: GPL-3.0-or-later
// Phase 45 hardens common multiples: +/-3pi and +/-4pi use deterministic aliases.

#include <math.h>

float sin_three_pi;
float cos_three_pi;
float sin_four_pi;
float cos_four_pi;

void main(void) {
    float angle;

    angle = 9.424778f;
    sin_three_pi = sinf(angle);
    cos_three_pi = cosf(angle);

    angle = 12.566371f;
    sin_four_pi = sinf(angle);
    cos_four_pi = cosf(angle);
}
