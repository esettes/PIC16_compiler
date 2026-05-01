// SPDX-License-Identifier: GPL-3.0-or-later

#include <pic16/pic16f628a.h>

union Value {
    unsigned char byte;
    unsigned int word;
};

void main(void) {
    union Value value;

    TRISB = 0x00;

    value.byte = 3;
    PORTB = value.byte;

    value.word = 0x0123;
    PORTB = (unsigned char)value.word;
}
