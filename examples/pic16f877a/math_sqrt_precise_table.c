// SPDX-License-Identifier: GPL-3.0-or-later

#include <math.h>

float root_quarter;
float root_eight;
float root_sixty_four;

void main(void) {
    float value;

    value = 0.25f;
    root_quarter = sqrtf(value);

    value = 8.0f;
    root_eight = sqrtf(value);

    value = 64.0f;
    root_sixty_four = sqrtf(value);
}
