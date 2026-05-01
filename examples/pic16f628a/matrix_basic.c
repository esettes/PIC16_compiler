// SPDX-License-Identifier: GPL-3.0-or-later

#include <pic16/pic16f628a.h>

unsigned char matrix[2][3] = {
    {1, 2, 3},
    {4, 5, 6}
};

void main(void) {
    unsigned char value;

    TRISB = 0x00;
    value = matrix[1][2];
    PORTB = value;
}
