#include <pic16/pic16f628a.h>

unsigned int final_value = 0;

/** Returns the sum of four 16-bit arguments using the Phase 4 call ABI. */
unsigned int sum4(unsigned int a, unsigned int b, unsigned int c, unsigned int d) {
    return a + b + c + d;
}

/** Sums a byte span with an explicit bias through pointer and length arguments. */
unsigned int sum_bytes(unsigned char *ptr, unsigned int len, unsigned int bias) {
    unsigned int i = 0;
    unsigned int acc = bias;

    while (i < len) {
        acc = acc + ptr[i];
        i = i + 1;
    }

    return acc;
}

/** Builds a per-call local array and forwards it through the new stack-backed ABI. */
unsigned int build_local(unsigned char base, unsigned char step, unsigned int bias) {
    unsigned char local[4];
    unsigned int i = 0;
    unsigned char value = base;

    while (i < sizeof(local)) {
        local[i] = value;
        value = value + step;
        i = i + 1;
    }

    return sum_bytes(local, sizeof(local), bias);
}

/** Exercises 3+ arguments, nested calls, and stack-backed locals on PIC16F628A. */
void main(void) {
    unsigned int value = 0;

    TRISB = 0x00;
    PORTB = 0x00;

    value = sum4(1, 2, build_local(3, 1, 2), 4);
    final_value = value;

    if (final_value >= 16) {
        PORTB = 0x3C;
    } else {
        PORTB = 0xC3;
    }
}
