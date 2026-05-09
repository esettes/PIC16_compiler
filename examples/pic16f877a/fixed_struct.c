#include <pic16/pic16f877a.h>

struct Calibration {
    __fixed8_8 offset;
    __ufixed8_8 gain;
};

struct Calibration cal;
__fixed8_8 corrected;

void main(void) {
    ADCON1 = 0x06;
    cal.offset = __q8_8(0x0080);
    cal.gain = __uq8_8(0x0200);
    corrected = (__fixed8_8)cal.gain + cal.offset;
    PORTB = (unsigned char)(int)corrected;
}
