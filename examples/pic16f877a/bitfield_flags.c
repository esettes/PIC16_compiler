// SPDX-License-Identifier: GPL-3.0-or-later

#include <pic16/pic16f877a.h>

struct Flags {
    unsigned char ready:1;
    unsigned char error:1;
    unsigned char mode:2;
    unsigned char spare:4;
};

void main(void) {
    struct Flags flags;

    ADCON1 = 0x06;
    TRISB = 0x00;

    flags.ready = 1;
    flags.error = 0;
    flags.mode = 2;

    if (flags.ready) {
        PORTB = flags.mode;
    }
}
