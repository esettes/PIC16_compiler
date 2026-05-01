// SPDX-License-Identifier: GPL-3.0-or-later

#include <pic16/pic16f877a.h>

struct Font {
    unsigned char glyph[2][3];
};

void main(void) {
    struct Font font;
    struct Font *ptr;

    ptr = &font;
    ptr->glyph[1][2] = 7;

    ADCON1 = 0x06;
    TRISB = 0x00;
    PORTB = font.glyph[1][2];
}
