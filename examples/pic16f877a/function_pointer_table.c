// SPDX-License-Identifier: GPL-3.0-or-later

#include <pic16/pic16f877a.h>

typedef unsigned char (*Transform)(unsigned char);

unsigned char plus_one(unsigned char value) {
    return value + 1;
}

unsigned char plus_two(unsigned char value) {
    return value + 2;
}

Transform table[2] = {plus_one, plus_two};

void main(void) {
    unsigned char index;
    unsigned char result;

    ADCON1 = 0x06;
    TRISB = 0x00;
    index = 1;
    result = table[index](3);
    PORTB = result;
}
