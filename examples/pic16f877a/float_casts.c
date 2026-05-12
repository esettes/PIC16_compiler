float from_int;
float from_fixed;
unsigned int whole;
__fixed8_8 q8;

void main(void) {
    from_int = (float)3;
    from_fixed = (float)1.5q8_8;
    whole = (unsigned int)3.75f;
    q8 = (__fixed8_8)1.5f;
}
