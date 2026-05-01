// SPDX-License-Identifier: GPL-3.0-or-later

#include <pic16/pic16f877a.h>

/** Multiplies two 16-bit values through the Phase 5 runtime helper path. */
unsigned int mul16(unsigned int a, unsigned int b) {
    return a * b;
}

/** Exercises 16-bit multiplication and helper-call lowering on PIC16F877A. */
void main(void) {
    unsigned int product = mul16(0x0012, 0x0007);
    ADCON1 = 0x06;
    TRISB = 0x00;
    PORTB = product;
}
// SPDX-License-Identifier: GPL-3.0-or-later
