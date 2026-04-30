#include <pic16/pic16f628a.h>

char message[] = "OK";

void main(void) {
    TRISB = 0x00;
    PORTB = message[0] + message[1];
}
