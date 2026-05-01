#include <pic16/pic16f877a.h>

const __rom char msg[] = "OK";

void main(void) {
    ADCON1 = 0x06;
    TRISB = 0x00;
    PORTB = __rom_read8(msg, 0) + __rom_read8(msg, 1);
    while (1) {
    }
}
