#include <math.h>

float angle;
float tangent;

void main(void) {
    /*
     * Suggested:
     * picc --target pic16f877a --math-profile balanced --size --memory-report \
     *      -I include -o build/math_tanf_resource_report.hex \
     *      examples/pic16f877a/math_tanf_resource_report.c
     */
    angle = 0.5235988f;
    tangent = tanf(angle);
}
