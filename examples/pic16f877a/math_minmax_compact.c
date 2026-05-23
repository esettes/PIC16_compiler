#include <math.h>

float low;
float high;

void main(void) {
    low = fminf(1.5f, 2.0f);
    high = fmaxf(-3.0f, -2.0f);
}
