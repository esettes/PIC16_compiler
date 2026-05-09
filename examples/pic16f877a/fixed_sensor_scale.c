#include <pic16/pic16f877a.h>

__ufixed8_8 adc_ratio = __uq8_8(0x0080);
__ufixed8_8 volts_per_count = __uq8_8(0x0500);
__ufixed8_8 scaled_voltage;
unsigned char display_value;

void main(void) {
    ADCON1 = 0x06;
    scaled_voltage = adc_ratio * volts_per_count;
    display_value = (unsigned char)(int)scaled_voltage;
    PORTB = display_value;
}
