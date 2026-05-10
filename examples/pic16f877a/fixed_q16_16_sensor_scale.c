#include <pic16/pic16f877a.h>

struct SensorScale {
    __fixed16_16 raw;
    __fixed16_16 gain;
};

struct SensorScale sensor;
__fixed16_16 scaled;

void main(void) {
    sensor.raw = 1.5q16_16;
    sensor.gain = 2.0q16_16;
    scaled = sensor.raw * sensor.gain;

    ADCON1 = 0x06;
    TRISB = 0x00;
    PORTB = (unsigned char)scaled;
}
