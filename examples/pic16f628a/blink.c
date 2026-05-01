// SPDX-License-Identifier: GPL-3.0-or-later

#include <pic16/pic16f628a.h>

/** Provides a small software delay for visible LED blinking on PORTB. */
void delay_tick(void) {
    unsigned char i = 8;
    while (i != 0) {
        i = i - 1;
    }
}

/** Configures PORTB as output and blinks a single bit forever. */
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
// SPDX-License-Identifier: GPL-3.0-or-later
