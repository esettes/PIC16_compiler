// SPDX-License-Identifier: GPL-3.0-or-later

#include <pic16/pic16f877a.h>

unsigned int words[3];
unsigned int mirror = 0;

/** Stores a 16-bit value into the second element of a word array. */
void store_word(unsigned int *ptr, unsigned int value) {
    ptr[1] = value;
}

/** Loads the second element of a word array through a data pointer. */
unsigned int load_word(unsigned int *ptr) {
    return ptr[1];
}

/** Exercises 16-bit arrays, pointer equality, sizeof, and indirect SFR writes. */
void main(void) {
    unsigned int local[2];
    unsigned int *cursor = words;
    volatile unsigned char *port = &PORTB;

    TRISB = 0x00;
    ADCON1 = 0x06;
    *port = 0x00;

    local[0] = sizeof(words);
    local[1] = 0;
    store_word(cursor, local[0]);
    mirror = load_word(words);

    if (cursor == words) {
        *port = 0x5A;
    }

    if (mirror >= local[0]) {
        *port = 0xA5;
    }
}
// SPDX-License-Identifier: GPL-3.0-or-later
