// SPDX-License-Identifier: GPL-3.0-or-later
// Phase 44: sinf/cosf share one internal finite trig core.

#include <math.h>

float angle;
float sine_value;
float cosine_value;

void main(void) {
    angle = 1.5707963f;
    sine_value = sinf(angle);
    cosine_value = cosf(angle);
}
