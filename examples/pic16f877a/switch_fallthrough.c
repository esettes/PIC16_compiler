// SPDX-License-Identifier: GPL-3.0-or-later

#include <pic16/pic16f877a.h>

void main(void) {
    unsigned char step = 1;
    unsigned char value = 0;

    ADCON1 = 0x06;
    TRISB = 0x00;

    switch (step) {
        case 0:
            value = 1;
            break;
        case 1:
            value = 10;
        case 2:
            value = value + 1;
            break;
        default:
            value = 0xFF;
            break;
    }

    PORTB = value;
}
// SPDX-License-Identifier: GPL-3.0-or-later
