#include <pic16/pic16f877a.h>

const unsigned char table[] = {1, 2, 3, 4};
static unsigned char flags[4];

void main(void) {
    ADCON1 = 0x06;
    TRISB = 0x00;
    flags[0] = table[0];
    flags[1] = table[3];
    PORTB = flags[0] + flags[1];
}
