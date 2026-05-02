// SPDX-License-Identifier: GPL-3.0-or-later

#include <pic16/pic16f877a.h>

unsigned int add3(unsigned int a, unsigned int b, unsigned int c) {
    unsigned char local[8];

    local[0] = 1;
    return a + b + c + local[0];
}

void main(void) {
    unsigned int total;

    ADCON1 = 0x06;
    TRISB = 0x00;
    total = add3(1, 2, 3);
    PORTB = (unsigned char)total;
}
