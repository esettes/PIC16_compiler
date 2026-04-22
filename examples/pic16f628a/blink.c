#include <pic16/pic16f628a.h>

void delay_tick(void) {
    unsigned char i = 8;
    while (i != 0) {
        i = i - 1;
    }
}

void main(void) {
    TRISB = 0x00;
    PORTB = 0x00;
    while (1) {
        PORTB = 0x01;
        delay_tick();
        PORTB = 0x00;
        delay_tick();
    }
}

