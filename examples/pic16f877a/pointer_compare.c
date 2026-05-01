#include <pic16/pic16f877a.h>

unsigned char bytes[4];

void main(void) {
    unsigned char *lhs = &bytes[1];
    const unsigned char *rhs = &bytes[2];

    ADCON1 = 0x06;
    TRISB = 0x00;

    if (lhs < rhs) {
        PORTB = 0x11;
    } else {
        PORTB = 0x22;
    }
}
