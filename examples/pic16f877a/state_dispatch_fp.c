// SPDX-License-Identifier: GPL-3.0-or-later

#include <pic16/pic16f877a.h>

typedef void (*StateHandler)(void);

void state_idle(void) {
    PORTB = 0x00;
}

void state_run(void) {
    PORTB = 0x01;
}

void state_error(void) {
    PORTB = 0xFF;
}

StateHandler states[3] = {state_idle, state_run, state_error};

void main(void) {
    unsigned char state;

    ADCON1 = 0x06;
    TRISB = 0x00;
    state = 2;
    states[state]();
}
