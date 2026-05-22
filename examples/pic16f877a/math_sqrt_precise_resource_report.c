// SPDX-License-Identifier: GPL-3.0-or-later

#include <math.h>

struct Measurement {
    float root;
};

struct Measurement measurement;

float compute_root(float value) {
    return sqrtf(value);
}

void main(void) {
    measurement.root = compute_root(10.0f);
}
