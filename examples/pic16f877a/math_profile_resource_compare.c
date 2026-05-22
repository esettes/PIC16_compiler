// SPDX-License-Identifier: GPL-3.0-or-later

#include <math.h>

float compact_input;
float compact_output;
float exact_output;

void main(void) {
    compact_input = 3.0f;
    compact_output = sqrtf(compact_input);
    exact_output = sqrtf(2.25f);
}
