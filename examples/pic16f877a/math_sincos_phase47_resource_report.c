// SPDX-License-Identifier: GPL-3.0-or-later
// Compare:
// picc --target pic16f877a --math-profile compact --size --memory-report -I include -o build/sincos_phase47_compact.hex examples/pic16f877a/math_sincos_phase47_resource_report.c
// picc --target pic16f877a --math-profile balanced --size --memory-report -I include -o build/sincos_phase47_balanced.hex examples/pic16f877a/math_sincos_phase47_resource_report.c

#include <math.h>

float angle;
float s;
float c;

void main(void) {
    angle = -25.132742f;
    s = sinf(angle);
    c = cosf(angle);
}
