// SPDX-License-Identifier: GPL-3.0-or-later

#include <pic16/pic16f877a.h>

struct Font {
    unsigned char glyph[2][3];
};

struct Font font = {
    .glyph[0][1] = 5,
    .glyph[1][2] = 9
};

void main(void) {
    ADCON1 = 0x06;
    TRISB = 0x00;
    PORTB = font.glyph[1][2];
}
