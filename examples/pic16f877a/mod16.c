// SPDX-License-Identifier: GPL-3.0-or-later

#include <pic16/pic16f877a.h>

/** Computes 16-bit modulo through the Phase 5 runtime helper path. */
unsigned int mod16(unsigned int a, unsigned int b) {
    return a % b;
}

/** Exercises unsigned 16-bit modulo and helper-call lowering on PIC16F877A. */
void main(void) {
    unsigned int remainder = mod16(1234, 17);
    ADCON1 = 0x06;
    TRISB = 0x00;
    PORTB = remainder;
}
// SPDX-License-Identifier: GPL-3.0-or-later
