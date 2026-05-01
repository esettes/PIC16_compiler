// SPDX-License-Identifier: GPL-3.0-or-later

#include <pic16/pic16f628a.h>

void main(void) {
    int signed_value = -1;
    unsigned int widened = (unsigned int)signed_value;
    unsigned char narrowed = (unsigned char)widened;
    unsigned int *word_ptr = (unsigned int *)0;
    unsigned char *byte_ptr = (unsigned char *)word_ptr;
    unsigned int addr = (unsigned int)byte_ptr;

    TRISB = 0x00;
    PORTB = narrowed + (unsigned char)addr;
}
// SPDX-License-Identifier: GPL-3.0-or-later
