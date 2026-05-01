// SPDX-License-Identifier: GPL-3.0-or-later

#include <pic16/pic16f877a.h>

const __rom unsigned char table[] = {0x10, 0x20, 0x40, 0x80};

void main(void) {
    unsigned char index = 0;

    ADCON1 = 0x06;
    TRISB = 0x00;

    while (1) {
        PORTB = __rom_read8(table, index);
        if (index < 3) {
            index = index + 1;
        } else {
            index = 0;
        }
    }
}
// SPDX-License-Identifier: GPL-3.0-or-later
