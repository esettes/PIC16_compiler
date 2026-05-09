#include <pic16/pic16f877a.h>

__fixed16_16 gain;
__fixed16_16 value;
__fixed16_16 product;
__fixed16_16 quotient;

void main(void) {
    gain = 1.5q16_16;
    value = 2.0q16_16;
    product = 1.5q16_16 * 2.0q16_16;
    quotient = 3.0q16_16 / 2.0q16_16;

    ADCON1 = 0x06;
    TRISB = 0x00;
    PORTB = (unsigned char)quotient;
}
