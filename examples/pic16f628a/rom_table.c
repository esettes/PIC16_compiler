// SPDX-License-Identifier: GPL-3.0-or-later

#include <pic16/pic16f628a.h>

const __rom unsigned char table[] = {1, 2, 3, 4};

void main(void) {
    TRISB = 0x00;
    PORTB = __rom_read8(table, 2);
    while (1) {
    }
}
// SPDX-License-Identifier: GPL-3.0-or-later
