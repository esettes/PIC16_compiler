// SPDX-License-Identifier: GPL-3.0-or-later

#include <pic16/pic16f628a.h>

unsigned char counter;

void main(void) {
    counter = 7;
    PORTB = counter;
}
