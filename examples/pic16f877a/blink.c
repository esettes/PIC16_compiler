// SPDX-License-Identifier: GPL-3.0-or-later

#include <pic16/pic16f877a.h>

/** Provides a small software delay for visible LED blinking on PORTC. */
void delay_tick(void) {
    unsigned char i = 16;
    while (i != 0) {
        i = i - 1;
    }
}

/** Configures digital I/O on PIC16F877A and blinks a PORTC bit forever. */
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
// SPDX-License-Identifier: GPL-3.0-or-later
