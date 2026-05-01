// SPDX-License-Identifier: GPL-3.0-or-later

#include <pic16/pic16f877a.h>

unsigned char matrix[2][3] = {
    {1},
    {4, 5}
};

void main(void) {
    ADCON1 = 0x06;
    TRISB = 0x00;
    PORTB = matrix[1][1];
}
