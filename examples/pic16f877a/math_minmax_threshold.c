// SPDX-License-Identifier: GPL-3.0-or-later

#include <math.h>

float sensor;
float lower_limit;
float upper_limit;
float clipped_low;
float clipped_high;

void main(void) {
    sensor = 3.0f;
    lower_limit = 1.0f;
    upper_limit = 2.0f;

    clipped_low = fmaxf(sensor, lower_limit);
    clipped_high = fminf(clipped_low, upper_limit);
}
