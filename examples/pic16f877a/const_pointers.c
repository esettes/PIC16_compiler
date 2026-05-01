#include <pic16/pic16f877a.h>

unsigned char led_mask = 0x01;
const unsigned char *mask_ptr = &led_mask;

void main(void) {
    unsigned char * const direct_ptr = &led_mask;
    const unsigned char * const readonly_ptr = &led_mask;

    ADCON1 = 0x06;
    TRISB = 0x00;

    mask_ptr = direct_ptr;
    PORTB = *mask_ptr + *readonly_ptr;
}
