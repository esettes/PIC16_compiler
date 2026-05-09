#include <pic16/pic16f877a.h>

const __rom __fixed8_8 calibration[] = {
    1.0q8_8,
    1.5q8_8,
    2.0q8_8
};

__fixed8_8 value;

void main(void) {
    unsigned char index;
    index = 1;
    value = calibration[index];

    ADCON1 = 0x06;
    TRISB = 0x00;
    PORTB = (unsigned char)value;
}
