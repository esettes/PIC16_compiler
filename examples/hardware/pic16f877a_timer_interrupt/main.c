// SPDX-License-Identifier: GPL-3.0-or-later

#include <pic16/pic16f877a.h>

#pragma config FOSC = HS
#pragma config WDTE = OFF
#pragma config PWRTE = ON
#pragma config BOREN = ON
#pragma config LVP = OFF
#pragma config CPD = OFF
#pragma config CP = OFF

volatile unsigned char ticks;

void __interrupt isr(void) {
    ticks = ticks + 1;
    PORTB = ticks;
}

void main(void) {
    ADCON1 = 0x06;
    TRISB = 0x00;
    PORTB = 0x00;
    ticks = 0;
    while (1) {
    }
}
