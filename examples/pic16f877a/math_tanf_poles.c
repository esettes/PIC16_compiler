#include <math.h>

float pos_pole;
float neg_pole;

void main(void) {
    /* Finite-only tangent saturates near odd pi/2 instead of returning Inf. */
    pos_pole = tanf(1.5707963f);
    neg_pole = tanf(-1.5707963f);
}
