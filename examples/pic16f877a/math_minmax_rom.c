// SPDX-License-Identifier: GPL-3.0-or-later

#include <math.h>

const __rom float calibration[] = { 1.25f, 2.5f };

float low;
float high;

void main(void) {
    float a = calibration[0];
    float b = calibration[1];

    low = fminf(a, b);
    high = fmaxf(a, b);
}
