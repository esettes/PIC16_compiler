// SPDX-License-Identifier: GPL-3.0-or-later

#include <pic16/pic16f877a.h>

enum Mode {
    MODE_IDLE,
    MODE_TX,
    MODE_RX = 8
};

void main(void) {
    enum Mode mode = MODE_TX;

    ADCON1 = 0x06;
    TRISB = 0x00;

    switch (mode) {
        case MODE_IDLE:
            PORTB = 0x00;
            break;
        case MODE_TX:
            PORTB = 0x55;
            break;
        default:
            PORTB = MODE_RX;
            break;
    }
}
// SPDX-License-Identifier: GPL-3.0-or-later
