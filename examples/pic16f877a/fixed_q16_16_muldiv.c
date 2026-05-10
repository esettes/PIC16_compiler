#include <pic16/pic16f877a.h>

__fixed16_16 a;
__fixed16_16 b;
__fixed16_16 product;
__fixed16_16 quotient;

void main(void) {
    a = 3.0q16_16;
    b = 2.0q16_16;
    product = 3.0q16_16 * 2.0q16_16;
    quotient = product / b;

    ADCON1 = 0x06;
    TRISB = 0x00;
    PORTB = (unsigned char)quotient;
}
