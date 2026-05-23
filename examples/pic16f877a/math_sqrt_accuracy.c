// SPDX-License-Identifier: GPL-3.0-or-later

#include <math.h>

float root_two;
float root_five;
float root_ten;

void main(void) {
    float value;

    value = 2.0f;
    root_two = sqrtf(value);

    value = 5.0f;
    root_five = sqrtf(value);

    value = 10.0f;
    root_ten = sqrtf(value);
}
