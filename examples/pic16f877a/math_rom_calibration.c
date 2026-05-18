// SPDX-License-Identifier: GPL-3.0-or-later

#include <math.h>

const __rom float offsets[] = {
    -1.25f,
    1.75f,
};

float corrected;

void main(void) {
    corrected = floorf(offsets[0]);
}
