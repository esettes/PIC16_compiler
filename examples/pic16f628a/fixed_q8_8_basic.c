#include <pic16/pic16f628a.h>

__fixed8_8 gain;
__fixed8_8 value;
__fixed8_8 result;

void main(void) {
    gain = (__fixed8_8)2;
    value = (__fixed8_8)3;
    result = gain * value;
    PORTB = (unsigned char)(int)result;
}
