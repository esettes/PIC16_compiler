#include <math.h>

const __rom float angles[] = {
    0.0f,
    1.5707963f,
    3.1415927f
};

float s;
float c;

void main(void) {
    float angle;
    angle = angles[1];
    s = sinf(angle);
    c = cosf(angle);
}
