// SPDX-License-Identifier: GPL-3.0-or-later

#include <pic16/pic16f628a.h>

#pragma config FOSC = INTRC_NOCLKOUT
#pragma config WDTE = OFF
#pragma config PWRTE = ON
#pragma config MCLRE = ON
#pragma config BOREN = ON
#pragma config LVP = OFF
#pragma config CPD = OFF
#pragma config CP = OFF

static void delay(void) {
    unsigned int outer;
    unsigned char inner;
    for (outer = 0; outer < 200; outer = outer + 1) {
        for (inner = 0; inner < 200; inner = inner + 1) {
        }
    }
}

void main(void) {
    TRISB = 0x00;
    PORTB = 0x00;
    while (1) {
        PORTB = 0x01;
        delay();
        PORTB = 0x00;
        delay();
    }
}
