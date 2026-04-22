#include <pic16/pic16f628a.h>

void main(void) {
    TRISB = 0x00;
    TMR0 = 0x00;
    while (1) {
        if (TMR0 != 0x00) {
            PORTB = PORTB ^ 0x01;
            TMR0 = 0x00;
        }
    }
}

