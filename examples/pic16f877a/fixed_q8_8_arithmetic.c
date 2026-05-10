#include <pic16/pic16f877a.h>

__fixed8_8 a = __q8_8(0x0180);
__fixed8_8 b = __q8_8(0x0200);
__fixed8_8 product;
__fixed8_8 quotient;
__fixed8_8 sum;

void main(void) {
    ADCON1 = 0x06;
    product = a * b;
    quotient = __q8_8(0x0300) / __q8_8(0x0200);
    sum = quotient + a;
    PORTB = (unsigned char)(int)sum;
}
