#include <pic16/pic16f628a.h>

struct Message {
    unsigned char text[4];
    unsigned char length;
};

struct Message message = {
    .text = "LED",
    .length = 3,
};

void main(void) {
    struct Message *cursor = &message;

    TRISB = 0x00;
    cursor->text[3] = 0;
    PORTB = cursor->text[0] + cursor->length;
}
