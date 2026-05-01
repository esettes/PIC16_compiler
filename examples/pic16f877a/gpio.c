// SPDX-License-Identifier: GPL-3.0-or-later

#include <pic16/pic16f877a.h>

/** Toggles the lowest PORTB bit in a tight loop after disabling analog input. */
void main(void) {
    ADCON1 = 0x06;
    TRISB = 0x00;
    PORTB = 0x00;
    while (1) {
        PORTB = PORTB ^ 0x01;
    }
}
// SPDX-License-Identifier: GPL-3.0-or-later
