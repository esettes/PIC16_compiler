// SPDX-License-Identifier: GPL-3.0-or-later

#include <math.h>

const __rom float calibration_roots[] = {
    0.25f,
    2.25f,
    9.0f,
};

float output;

void main(void) {
    float sample = calibration_roots[1];
    output = sqrtf(sample);
}
