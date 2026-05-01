// SPDX-License-Identifier: GPL-3.0-or-later

#include <pic16/pic16f877a.h>

unsigned int words[3];

void main(void) {
    unsigned int *lhs = &words[2];
    unsigned int *rhs = &words[0];
    int diff = lhs - rhs;

    ADCON1 = 0x06;
    TRISB = 0x00;

    PORTB = (unsigned char)diff;
}
// SPDX-License-Identifier: GPL-3.0-or-later
