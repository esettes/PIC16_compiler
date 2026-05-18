// SPDX-License-Identifier: GPL-3.0-or-later

#include <math.h>

float input;
float absolute_value;
float integer_part;

void main(void) {
    input = -1.75f;
    absolute_value = fabsf(input);
    integer_part = truncf(input);
}
