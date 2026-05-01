// SPDX-License-Identifier: GPL-3.0-or-later

#include <pic16/pic16f877a.h>

volatile unsigned char edge_count = 0;

void __interrupt isr(void) {
    if ((INTCON & 0x02) != 0) {
        edge_count = edge_count + 1;
        PORTB = PORTB ^ 0x04;
        INTCON = INTCON & 0xFD;
    }
}

void main(void) {
    ADCON1 = 0x06;
    TRISB = 0x01;
    PORTB = 0x00;
    INTCON = 0x90;

    while (1) {
        if ((edge_count & 0x01) != 0) {
            PORTB = PORTB ^ 0x08;
            edge_count = 0;
        }
    }
}
// SPDX-License-Identifier: GPL-3.0-or-later
