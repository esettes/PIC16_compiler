// SPDX-License-Identifier: GPL-3.0-or-later

#include <pic16/pic16f877a.h>

int bias = -3;
int latest = 0;

/** Returns a signed 16-bit subtraction used by the comparison example. */
int adjust16(int base, int step) {
    return base - step;
}

/** Exercises signed 16-bit comparisons and loop control on PIC16F877A. */
void main(void) {
    int value = adjust16(bias, 5);

    TRISB = 0x00;
    ADCON1 = 0x06;
    PORTB = 0x00;

    if (value < 0) {
        PORTB = 0xF0;
    }

    while (value <= 3) {
        value = value + 1;
    }

    latest = value;
    if (!(latest > 3)) {
        PORTB = 0x33;
    } else {
        PORTB = 0x0F;
    }
}
// SPDX-License-Identifier: GPL-3.0-or-later
