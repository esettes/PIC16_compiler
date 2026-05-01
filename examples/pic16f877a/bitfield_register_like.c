// SPDX-License-Identifier: GPL-3.0-or-later

#include <pic16/pic16f877a.h>

struct Control {
    unsigned char enable:1;
    unsigned char mode:2;
    unsigned char latch:1;
    unsigned char spare:4;
};

void main(void) {
    volatile struct Control control;

    ADCON1 = 0x06;
    TRISB = 0x00;

    control.enable = 1;
    control.mode = 3;
    control.latch = control.enable;

    PORTB = control.mode;
}
