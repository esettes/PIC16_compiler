// SPDX-License-Identifier: GPL-3.0-or-later

#include <pic16/pic16f877a.h>

const __rom unsigned int table16[] = {100, 200, 300};

unsigned int read_value(unsigned char index) {
    return table16[index];
}

void main(void) {
    unsigned int value;

    ADCON1 = 0x06;
    TRISB = 0x00;
    value = read_value(1);
    PORTB = (unsigned char)value;
}
