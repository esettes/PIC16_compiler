// SPDX-License-Identifier: GPL-3.0-or-later

#include <pic16/pic16f877a.h>

struct PinConfig {
    unsigned char port;
    unsigned char bit;
};

struct DeviceConfig {
    struct PinConfig led;
    unsigned char name[4];
};

struct DeviceConfig configs[2] = {
    {{1, 0}, "LED"},
    {
        .led = {2, 3},
        .name = "BTN",
    },
};

void main(void) {
    ADCON1 = 0x06;
    TRISB = 0x00;
    PORTB = configs[1].led.bit + configs[0].name[0];
}
// SPDX-License-Identifier: GPL-3.0-or-later
