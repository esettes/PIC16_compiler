// SPDX-License-Identifier: GPL-3.0-or-later

#include <pic16/pic16f628a.h>

/** Polls TMR0 and toggles PORTB whenever the timer advances from zero. */
void main(void) {
    TRISB = 0x00;
    TMR0 = 0x00;
    while (1) {
        if (TMR0 != 0x00) {
            PORTB = PORTB ^ 0x01;
            TMR0 = 0x00;
        }
    }
}
// SPDX-License-Identifier: GPL-3.0-or-later
