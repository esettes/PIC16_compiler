// SPDX-License-Identifier: GPL-3.0-or-later
// Phase 45: finite trig accuracy grid in radians.
// Compact tolerance: <= 0.10. Balanced tolerance: <= 0.05.

#include <math.h>

float angle;
float sin_pi6;
float cos_pi6;
float sin_pi3;
float cos_pi3;

void main(void) {
    angle = 0.5235988f;
    sin_pi6 = sinf(angle);
    cos_pi6 = cosf(angle);

    angle = 1.0471976f;
    sin_pi3 = sinf(angle);
    cos_pi3 = cosf(angle);
}
