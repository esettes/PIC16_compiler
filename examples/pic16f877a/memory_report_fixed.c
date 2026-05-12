// SPDX-License-Identifier: GPL-3.0-or-later

#include <pic16/pic16f877a.h>

__fixed16_16 input;
__fixed16_16 gain;
__fixed16_16 scaled;

void main(void) {
    input = 1.5q16_16;
    gain = 2.0q16_16;
    scaled = input * gain;
    PORTB = (unsigned char)((long)scaled);
}
