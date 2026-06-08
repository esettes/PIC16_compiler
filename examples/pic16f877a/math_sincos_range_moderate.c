// SPDX-License-Identifier: GPL-3.0-or-later
// Phase 46: deterministic moderate aliases through +/-8pi.

#include <math.h>

float angle;
float sin_five_pi;
float cos_five_pi;
float sin_neg_six_pi;
float cos_neg_six_pi;

void main(void) {
    angle = 15.707963f;
    sin_five_pi = sinf(angle);
    cos_five_pi = cosf(angle);

    angle = -18.849556f;
    sin_neg_six_pi = sinf(angle);
    cos_neg_six_pi = cosf(angle);
}
