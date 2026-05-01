// SPDX-License-Identifier: GPL-3.0-or-later

#include <pic16/pic16f628a.h>

unsigned char shadow[4];
unsigned char total = 0;

/** Exercises array decay, pointer indexing, and `sizeof` on a byte array. */
void main(void) {
    unsigned int i = 0;
    unsigned char *cursor = shadow;

    TRISB = 0x00;
    PORTB = 0x00;

    while (i < sizeof(shadow)) {
        cursor[i] = 1;
        total = total + cursor[i];
        i = i + 1;
    }

    if (shadow[2] != 0) {
        PORTB = total;
    } else {
        PORTB = 0xFF;
    }
}
// SPDX-License-Identifier: GPL-3.0-or-later
