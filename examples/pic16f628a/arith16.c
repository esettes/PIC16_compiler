#include <pic16/pic16f628a.h>

unsigned int threshold = 300;
unsigned int counter = 0;
unsigned int result = 0;

/** Returns the 16-bit sum used by the Phase 2 arithmetic example. */
unsigned int add16(unsigned int lhs, unsigned int rhs) {
    return lhs + rhs;
}

/** Exercises 16-bit locals, calls, subtraction, and unsigned relations on PIC16F628A. */
void main(void) {
    unsigned int local = 0;

    TRISB = 0x00;
    PORTB = 0x00;

    result = add16(threshold, 42);
    local = result - 1;

    while (local < result) {
        local = local + 1;
    }

    counter = local;
    if (counter >= result) {
        PORTB = 0x55;
    } else {
        PORTB = 0xAA;
    }
}
