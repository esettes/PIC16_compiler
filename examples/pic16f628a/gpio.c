#include <pic16/pic16f628a.h>

void main(void) {
    TRISB = 0xF0;
    PORTB = 0x00;
    while (1) {
        PORTB = PORTB ^ 0x01;
    }
}

