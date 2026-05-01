// SPDX-License-Identifier: GPL-3.0-or-later

#include <pic16/pic16f877a.h>

/** Mixes one inline constant shift with one runtime-count shift. */
unsigned int shift_mix(unsigned int a, unsigned char n) {
    return (a << 1) + (a >> n);
}

/** Exercises inline and helper-driven shifts on PIC16F877A. */
void main(void) {
    unsigned int mixed = shift_mix(0x0123, 3);
    ADCON1 = 0x06;
    TRISB = 0x00;
    PORTB = mixed;
}
// SPDX-License-Identifier: GPL-3.0-or-later
