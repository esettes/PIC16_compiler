// SPDX-License-Identifier: GPL-3.0-or-later

#include <pic16/pic16f877a.h>

/*
 * Phase 18 diagnostic example.
 * Mutual recursion still rejected in this phase.
 */
unsigned char odd(unsigned char value);

unsigned char even(unsigned char value) {
    if (value == 0) {
        return 1;
    }
    return odd(value - 1);
}

unsigned char odd(unsigned char value) {
    if (value == 0) {
        return 0;
    }
    return even(value - 1);
}

void main(void) {
    ADCON1 = 0x06;
    TRISB = 0x00;
    PORTB = even(4);
}
