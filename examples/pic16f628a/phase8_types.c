// SPDX-License-Identifier: GPL-3.0-or-later

#include <pic16/pic16f628a.h>

typedef unsigned char u8;
typedef unsigned int u16;

enum Mode {
    MODE_OFF,
    MODE_ON,
    MODE_ERROR = 10
};

struct Point {
    u8 x;
    u8 y;
};

u8 samples[3] = {1, 2};
struct Point origin = {1};

u8 encode_mode(enum Mode mode) {
    if (mode == MODE_ERROR) {
        return MODE_ON;
    }
    return MODE_OFF;
}

void main(void) {
    u16 wide = 300;
    u8 narrowed = (u8)wide;

    TRISB = 0x00;
    PORTB = narrowed + samples[2] + origin.y + encode_mode(MODE_ERROR);
}
// SPDX-License-Identifier: GPL-3.0-or-later
