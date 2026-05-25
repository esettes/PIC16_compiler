#include <math.h>

float angle;
float sine_value;
float cosine_value;

void main(void) {
    angle = 3.1415927f;
    sine_value = sinf(angle);
    cosine_value = cosf(angle);
}
