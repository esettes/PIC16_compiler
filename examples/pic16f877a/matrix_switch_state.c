// SPDX-License-Identifier: GPL-3.0-or-later

#include <pic16/pic16f877a.h>

unsigned char matrix[2][3] = {
    {0x10, 0x11, 0x12},
    {0x20, 0x21, 0x22}
};

void main(void) {
    unsigned char state;

    ADCON1 = 0x06;
    TRISB = 0x00;
    state = 1;

    switch (state) {
        case 0:
            PORTB = matrix[0][2];
            break;
        default:
            PORTB = matrix[1][1];
            break;
    }
}
