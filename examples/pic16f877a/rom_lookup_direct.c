// SPDX-License-Identifier: GPL-3.0-or-later

#include <pic16/pic16f877a.h>

const __rom unsigned char lookup[] = {0x10, 0x20, 0x40, 0x80};

void main(void) {
    unsigned char index;
    unsigned char value;

    ADCON1 = 0x06;
    TRISB = 0x00;
    index = 3;
    value = lookup[index];
    PORTB = value;
}
