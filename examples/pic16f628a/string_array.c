// SPDX-License-Identifier: GPL-3.0-or-later

#include <pic16/pic16f628a.h>

char message[] = "OK";

void main(void) {
    TRISB = 0x00;
    PORTB = message[0] + message[1];
}
// SPDX-License-Identifier: GPL-3.0-or-later
