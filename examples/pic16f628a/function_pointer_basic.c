// SPDX-License-Identifier: GPL-3.0-or-later

#include <pic16/pic16f628a.h>

typedef void (*Handler)(void);

void off(void) {
    PORTB = 0x00;
}

void on(void) {
    PORTB = 0x01;
}

void main(void) {
    Handler handler;

    TRISB = 0x00;
    handler = on;
    handler();
    handler = off;
    handler();
}
