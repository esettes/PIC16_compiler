#include <pic16/pic16f877a.h>

void main(void) {
    ADCON1 = 0x06;
    TRISB = 0x00;
    PORTB = 0x00;
    while (1) {
        PORTB = PORTB ^ 0x01;
    }
}

