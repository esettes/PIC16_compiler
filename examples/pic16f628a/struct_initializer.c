// SPDX-License-Identifier: GPL-3.0-or-later

#include <pic16/pic16f628a.h>

struct Point {
    unsigned char x;
    unsigned char y;
};

struct Point origin = {7};

void main(void) {
    struct Point local = {3};

    TRISB = 0x00;
    PORTB = origin.x + origin.y + local.x + local.y;
}
// SPDX-License-Identifier: GPL-3.0-or-later
