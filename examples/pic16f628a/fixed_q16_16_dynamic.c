#include <pic16/pic16f628a.h>

__fixed16_16 raw_value;
__fixed16_16 gain;
__fixed16_16 value;

void main(void) {
    raw_value = 1.5q16_16;
    gain = 2.0q16_16;
    value = raw_value + gain;

    TRISB = 0x00;
    PORTB = (unsigned char)value;
}
