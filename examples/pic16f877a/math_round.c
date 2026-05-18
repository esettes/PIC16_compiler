// SPDX-License-Identifier: GPL-3.0-or-later

#include <math.h>

float rounded_positive;
float rounded_negative;

void main(void) {
    rounded_positive = roundf(1.5f);
    rounded_negative = roundf(-1.5f);
}
