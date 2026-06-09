#include <math.h>

float angle;
float tangent;

void main(void) {
    /* Compile with --math-profile compact and balanced to compare cost/tolerance. */
    angle = 1.0471976f;
    tangent = tanf(angle);
}
