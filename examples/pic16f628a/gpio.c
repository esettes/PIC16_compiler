// SPDX-License-Identifier: GPL-3.0-or-later

#include <pic16/pic16f628a.h>

/** Toggles the lowest PORTB bit in a tight loop to exercise GPIO writes. */
void main(void) {
    TRISB = 0xF0;
    PORTB = 0x00;
    while (1) {
        PORTB = PORTB ^ 0x01;
    }
}
// SPDX-License-Identifier: GPL-3.0-or-later
