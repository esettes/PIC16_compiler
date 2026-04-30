#include <pic16/pic16f877a.h>

struct Pair {
    unsigned char lo;
    unsigned char hi;
};

unsigned char message[] = "HI";
unsigned int ticks = 0x1234;
static unsigned char flags[4];
struct Pair pair = {1};

void main(void) {
    ADCON1 = 0x06;
    TRISB = 0x00;
    PORTB = message[0] + pair.hi + flags[0] + (unsigned char)ticks;
}
