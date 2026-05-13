__fixed16_16 fixed_value;
float float_value;
__fixed16_16 result;

void main(void) {
    fixed_value = -2.25q16_16;
    float_value = (float)fixed_value;
    result = (__fixed16_16)float_value;
}
