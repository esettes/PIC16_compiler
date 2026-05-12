// SPDX-License-Identifier: GPL-3.0-or-later

#include <pic16/pic16f877a.h>

const __rom __fixed8_8 calibration[] = {
    1.0q8_8,
    1.5q8_8,
    2.0q8_8,
};

__fixed8_8 value;

void main(void) {
    value = calibration[1];
    PORTB = (unsigned char)((int)value);
}
