// SPDX-License-Identifier: GPL-3.0-or-later

#include <pic16/pic16f877a.h>

/** Combines multiply, divide, and modulo in one expression tree. */
unsigned int expression_test(unsigned int a, unsigned int b, unsigned int c) {
    return (a * b) + (c / 3) - (a % 5);
}

/** Exercises nested helper calls inside one expression on PIC16F877A. */
void main(void) {
    unsigned int value = expression_test(9, 11, 27);
    ADCON1 = 0x06;
    TRISB = 0x00;
    PORTB = value;
}
// SPDX-License-Identifier: GPL-3.0-or-later
