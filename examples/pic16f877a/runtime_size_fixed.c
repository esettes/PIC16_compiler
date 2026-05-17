__fixed16_16 raw;
__fixed16_16 gain;
__fixed16_16 result;

void main(void) {
    raw = 1.5q16_16;
    gain = 2.0q16_16;
    result = raw * gain;
}
