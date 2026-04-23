#include <pic16/pic16f877a.h>

unsigned int latest = 0;

/** Returns the sum of three arguments to exercise caller-pushed parameters. */
unsigned int leaf_sum(unsigned int a, unsigned int b, unsigned int c) {
    return a + b + c;
}

/** Builds a local 16-bit array and forwards its values through a deeper call chain. */
unsigned int middle_sum(unsigned int base, unsigned int step) {
    unsigned int local[2];

    local[0] = base;
    local[1] = step;
    return leaf_sum(local[0], local[1], base + step);
}

/** Drives nested calls and updates a caller-provided port pointer when work completes. */
unsigned int top_sum(unsigned int seed, volatile unsigned char *port) {
    unsigned int temp = middle_sum(seed, 1) + middle_sum(seed, 2);

    if (temp != 0) {
        *port = 0x5A;
    }

    return temp;
}

/** Exercises deep non-recursive call chains, local arrays, and pointer arguments on PIC16F877A. */
void main(void) {
    volatile unsigned char *port = &PORTB;

    TRISB = 0x00;
    ADCON1 = 0x06;
    *port = 0x00;

    latest = top_sum(5, port);
    if (latest >= 10) {
        *port = 0xA5;
    }
}
