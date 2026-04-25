#include <pic16/pic16f628a.h>

volatile unsigned char tick_count = 0;

void __interrupt isr(void) {
    if ((INTCON & 0x04) != 0) {
        tick_count = tick_count + 1;
        PORTB = PORTB ^ 0x01;
        INTCON = INTCON & 0xFB;
    }
}

void main(void) {
    TRISB = 0x00;
    PORTB = 0x00;
    INTCON = 0xA0;

    while (1) {
        if ((tick_count & 0x08) != 0) {
            PORTB = PORTB ^ 0x02;
            tick_count = 0;
        }
    }
}
