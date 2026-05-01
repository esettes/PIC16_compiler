// SPDX-License-Identifier: GPL-3.0-or-later

#include <pic16/pic16f628a.h>

const __rom unsigned char table[] = {1, 2, 3, 4};

void main(void) {
    unsigned char i;
    unsigned char value;

    TRISB = 0x00;
    i = 2;
    value = table[i];
    PORTB = value;
}
