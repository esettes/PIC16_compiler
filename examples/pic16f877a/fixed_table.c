#include <pic16/pic16f877a.h>

__fixed8_8 table[3] = {
    __q8_8(0x0100),
    __q8_8(0x0180),
    __q8_8(0x0200)
};
unsigned char index;
__fixed8_8 selected;

void main(void) {
    ADCON1 = 0x06;
    index = 1;
    selected = table[index];
    PORTB = (unsigned char)(int)selected;
}
