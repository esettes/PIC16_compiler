#include <pic16/pic16f877a.h>

char *message = "OK";
const char *banner = "HI";

void main(void) {
    ADCON1 = 0x06;
    TRISB = 0x00;

    PORTB = (unsigned char)message[0] + (unsigned char)banner[1];
}
