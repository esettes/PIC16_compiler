// SPDX-License-Identifier: GPL-3.0-or-later

#include <pic16/pic16f877a.h>

struct Pair {
    unsigned char lo;
    unsigned int hi;
};

struct Pair global_pair;

unsigned char touch(struct Pair *pair) {
    pair->lo = 3;
    pair->hi = 0x1234;
    return pair->lo;
}

void main(void) {
    struct Pair local = {1, 2};
    struct Pair *cursor = &local;

    ADCON1 = 0x06;
    TRISB = 0x00;

    global_pair.lo = touch(cursor);
    PORTB = global_pair.lo;
}
// SPDX-License-Identifier: GPL-3.0-or-later
