#include <pic16/pic16f877a.h>

void delay_tick(void) {
    unsigned char i = 16;
    while (i != 0) {
        i = i - 1;
    }
}

void main(void) {
    ADCON1 = 0x06;
    TRISC = 0x00;
    PORTC = 0x00;
    while (1) {
        PORTC = 0x01;
        delay_tick();
        PORTC = 0x00;
        delay_tick();
    }
}

