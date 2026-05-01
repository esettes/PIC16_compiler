// SPDX-License-Identifier: GPL-3.0-or-later

#include <pic16/pic16f628a.h>

unsigned char value;
unsigned char *ptr = &value;
unsigned char **ptrptr = &ptr;

void write_value(unsigned char **slot, unsigned char next) {
    **slot = next;
}

void main(void) {
    TRISB = 0x00;

    write_value(ptrptr, 3);
    PORTB = value;
}
// SPDX-License-Identifier: GPL-3.0-or-later
