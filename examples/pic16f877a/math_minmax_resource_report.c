// SPDX-License-Identifier: GPL-3.0-or-later
//
// Useful with:
//   picc --target pic16f877a --size --memory-report -I include -o build/minmax.hex \
//        examples/pic16f877a/math_minmax_resource_report.c

#include <math.h>

float a;
float b;
float low;
float high;

void main(void) {
    a = -1.0f;
    b = 2.0f;

    low = fminf(a, b);
    high = fmaxf(a, b);
}
