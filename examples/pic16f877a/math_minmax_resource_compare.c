#include <math.h>

float sensor;
float lower_limit;
float upper_limit;
float clamped;

void main(void) {
    sensor = 2.75f;
    lower_limit = 1.0f;
    upper_limit = 2.5f;

    clamped = fmaxf(sensor, lower_limit);
    clamped = fminf(clamped, upper_limit);
}
