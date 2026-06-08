// SPDX-License-Identifier: GPL-3.0-or-later
// Report:
// picc --target pic16f877a --math-profile balanced --size --memory-report -I include -o build/sincos_range_report.hex examples/pic16f877a/math_sincos_range_resource_report.c

#include <math.h>

float angle;
float sin_out;
float cos_out;

void main(void) {
    angle = -25.132742f;
    sin_out = sinf(angle);
    cos_out = cosf(angle);
}
