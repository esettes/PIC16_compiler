// SPDX-License-Identifier: GPL-3.0-or-later

#include <pic16/pic16f628a.h>

unsigned int scale(unsigned int value) {
    return value * 3;
}

unsigned int blend(unsigned int left, unsigned int right) {
    unsigned int mixed;

    mixed = scale(left);
    return mixed + right;
}

void main(void) {
    unsigned int result;

    TRISB = 0x00;
    result = blend(2, 5);
    PORTB = (unsigned char)result;
}
