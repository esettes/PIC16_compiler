// SPDX-License-Identifier: GPL-3.0-or-later

#include <math.h>

float low;
float high;

void main(void) {
    float a = 1.5f;
    float b = 2.0f;

    low = fminf(a, b);
    high = fmaxf(a, b);
}
