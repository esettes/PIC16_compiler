// SPDX-License-Identifier: GPL-3.0-or-later

#include <pic16/pic16f877a.h>

/*
 * Phase 18 diagnostic example.
 * Recursion still rejected in this phase, even with --stack-check.
 */
unsigned char countdown(unsigned char depth) {
    if (depth == 0) {
        return 0;
    }
    return countdown(depth - 1);
}

void main(void) {
    ADCON1 = 0x06;
    TRISB = 0x00;
    PORTB = countdown(3);
}
