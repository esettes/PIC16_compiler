// SPDX-License-Identifier: GPL-3.0-or-later

#include <math.h>

float input;
float output;

void main(void) {
    input = -1.5f;
    output = roundf(fabsf(input));
}
