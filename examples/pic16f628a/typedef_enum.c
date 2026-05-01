// SPDX-License-Identifier: GPL-3.0-or-later

#include <pic16/pic16f628a.h>

typedef unsigned char u8;
typedef unsigned int u16;

enum Mode {
    MODE_OFF,
    MODE_ON = 3,
    MODE_ERROR = 7
};

u8 encode_mode(enum Mode mode) {
    if (mode == MODE_ERROR) {
        return MODE_ON;
    }
    return MODE_OFF;
}

void main(void) {
    u16 raw = MODE_ERROR;
    u8 narrowed = (u8)raw;

    TRISB = 0x00;
    PORTB = narrowed + encode_mode(MODE_ERROR);
}
// SPDX-License-Identifier: GPL-3.0-or-later
