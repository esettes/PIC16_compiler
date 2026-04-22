#include <pic16/pic16f877a.h>

void main(void) {
    ADCON1 = 0x06;
    TRISD = 0x00;
    TMR0 = 0x00;
    while (1) {
        if (TMR0 != 0x00) {
            PORTD = PORTD ^ 0x01;
            TMR0 = 0x00;
        }
    }
}

