// SPDX-License-Identifier: GPL-3.0-or-later

#include <pic16/pic16f877a.h>

struct Point {
    unsigned char x;
    unsigned char y;
};

struct Box {
    struct Point top_left;
    struct Point bottom_right;
};

struct Box box = {
    {1, 2},
    {3, 4},
};

void main(void) {
    ADCON1 = 0x06;
    TRISB = 0x00;
    PORTB = box.top_left.x + box.bottom_right.y;
}
// SPDX-License-Identifier: GPL-3.0-or-later
