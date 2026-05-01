// SPDX-License-Identifier: GPL-3.0-or-later

#include <pic16/pic16f877a.h>

struct Point {
    unsigned char x;
    unsigned char y;
};

struct Point point = {
    .y = 2,
    .x = 1,
};

unsigned char table[4] = {
    [0] = 1,
    [3] = 9,
};

void main(void) {
    ADCON1 = 0x06;
    TRISB = 0x00;
    PORTB = point.x + point.y + table[3];
}
// SPDX-License-Identifier: GPL-3.0-or-later
