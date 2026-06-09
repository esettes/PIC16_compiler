// SPDX-License-Identifier: GPL-3.0-or-later
// Phase 47: positive and negative moderate aliases share one magnitude match.

#include <math.h>

float sin_pos;
float sin_neg;
float cos_pos;
float cos_neg;

void main(void) {
    float angle;

    angle = 15.707963f;
    sin_pos = sinf(angle);
    cos_pos = cosf(angle);

    angle = -15.707963f;
    sin_neg = sinf(angle);
    cos_neg = cosf(angle);
}
