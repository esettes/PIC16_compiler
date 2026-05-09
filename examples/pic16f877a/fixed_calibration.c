#include <pic16/pic16f877a.h>

const __rom __ufixed16_16 gains[] = {
    1.0uq16_16,
    0.5uq16_16,
    2.0uq16_16
};

__ufixed16_16 gain;
__ufixed16_16 scaled;

void main(void) {
    unsigned char index;
    index = 1;
    gain = gains[index];
    scaled = gain + 1.5uq16_16;

    ADCON1 = 0x06;
    TRISB = 0x00;
    PORTB = (unsigned char)scaled;
}
