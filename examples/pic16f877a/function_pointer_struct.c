// SPDX-License-Identifier: GPL-3.0-or-later

#include <pic16/pic16f877a.h>

typedef void (*Handler)(void);

struct Device {
    Handler handler;
};

void led_off(void) {
    PORTB = 0x00;
}

void led_on(void) {
    PORTB = 0x01;
}

void main(void) {
    struct Device device;

    ADCON1 = 0x06;
    TRISB = 0x00;
    device.handler = led_on;
    device.handler();
    device.handler = led_off;
    device.handler();
}
