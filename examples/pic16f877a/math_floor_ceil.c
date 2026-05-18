// SPDX-License-Identifier: GPL-3.0-or-later

#include <math.h>

float low;
float high;

void main(void) {
    low = floorf(-1.25f);
    high = ceilf(1.25f);
}
