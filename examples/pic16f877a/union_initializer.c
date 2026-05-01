// SPDX-License-Identifier: GPL-3.0-or-later

#include <pic16/pic16f877a.h>

union Value {
    unsigned char byte;
    unsigned int word;
};

union Value first = {3};
union Value selected = {.word = 1000};

void main(void) {
    ADCON1 = 0x06;
    TRISB = 0x00;
    PORTB = first.byte + (unsigned char)selected.word;
}
