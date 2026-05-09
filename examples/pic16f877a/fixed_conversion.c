#include <pic16/pic16f877a.h>

__fixed8_8 q8_value;
__fixed16_16 q16_value;
int whole;

void main(void) {
    q8_value = 1.5q8_8;
    q16_value = (__fixed16_16)q8_value;
    q8_value = (__fixed8_8)2.25q16_16;
    whole = (int)q8_value;

    ADCON1 = 0x06;
    TRISB = 0x00;
    PORTB = (unsigned char)whole;
}
