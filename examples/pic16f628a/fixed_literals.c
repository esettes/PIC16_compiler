#include <pic16/pic16f628a.h>

__fixed8_8 q8_value;
__ufixed8_8 uq8_value;
__fixed16_16 q16_value;

void main(void) {
    q8_value = 1.5q8_8;
    uq8_value = 2.25uq8_8;
    q16_value = 10.125q16_16;

    TRISB = 0x00;
    PORTB = (unsigned char)(q8_value);
}
