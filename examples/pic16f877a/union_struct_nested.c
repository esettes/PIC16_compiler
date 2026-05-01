// SPDX-License-Identifier: GPL-3.0-or-later

#include <pic16/pic16f877a.h>

union Payload {
    unsigned char byte;
    unsigned int word;
};

struct Packet {
    unsigned char id;
    union Payload payload;
};

void main(void) {
    struct Packet packet;

    ADCON1 = 0x06;
    TRISB = 0x00;

    packet.id = 1;
    packet.payload.word = 0x0034;
    PORTB = packet.payload.byte + packet.id;
}
