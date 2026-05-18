__fixed8_8 q8_a;
__fixed8_8 q8_b;
__fixed8_8 q8_result;

float f_a;
float f_b;
float f_sum;
float f_result;

void main(void) {
    q8_a = 2.0q8_8;
    q8_b = 1.5q8_8;
    q8_result = q8_a + q8_b;

    f_a = 4.0f;
    f_b = 1.5f;
    f_sum = f_a + f_b;
    f_result = f_a - f_b;
}
