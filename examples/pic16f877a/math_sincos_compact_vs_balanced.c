// SPDX-License-Identifier: GPL-3.0-or-later
// Compare:
// picc --target pic16f877a --math-profile compact --size --memory-report -I include -o build/trig_compact.hex examples/pic16f877a/math_sincos_compact_vs_balanced.c
// picc --target pic16f877a --math-profile balanced --size --memory-report -I include -o build/trig_balanced.hex examples/pic16f877a/math_sincos_compact_vs_balanced.c

#include <math.h>

float angle;
float s;
float c;

void main(void) {
    angle = 2.6179938f;
    s = sinf(angle);
    c = cosf(angle);
}
