#include <pic16/pic16f877a.h>

/** Divides two signed 16-bit values through the Phase 5 runtime helper path. */
int div16(int a, int b) {
    return a / b;
}

/** Exercises signed 16-bit division and runtime-helper lowering on PIC16F877A. */
void main(void) {
    int quotient = div16(-120, 7);
    ADCON1 = 0x06;
    TRISB = 0x00;
    PORTB = quotient;
}
