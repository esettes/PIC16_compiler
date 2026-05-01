// SPDX-License-Identifier: GPL-3.0-or-later

#include <pic16/pic16f628a.h>

unsigned char global_values[4] = {1, 2};

void main(void) {
    unsigned char local_values[3] = {5};

    TRISB = 0x00;
    PORTB = global_values[0]
        + global_values[1]
        + global_values[2]
        + global_values[3]
        + local_values[0]
        + local_values[1]
        + local_values[2];
}
// SPDX-License-Identifier: GPL-3.0-or-later
