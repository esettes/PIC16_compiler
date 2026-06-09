// SPDX-License-Identifier: GPL-3.0-or-later
// Phase 47: shared sin/cos core uses sign-normalized aliases to recover size.
// picc --target pic16f877a --math-profile balanced --size --memory-report -I include -o build/sincos_core_size_recovery.hex examples/pic16f877a/math_sincos_core_size_recovery.c

#include <math.h>

float angle;
float s;
float c;

void main(void) {
    angle = 25.132742f;
    s = sinf(angle);
    c = cosf(angle);
}
