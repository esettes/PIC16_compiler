#include <pic16/pic16f877a.h>

struct Config {
    unsigned char mode;
    unsigned int limit;
};

const struct Config config = {1, 0x1234};

void main(void) {
    ADCON1 = 0x06;
    TRISB = 0x00;
    PORTB = config.mode + (unsigned char)config.limit;
}
