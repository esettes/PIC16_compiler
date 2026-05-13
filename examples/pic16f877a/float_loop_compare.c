float value;
float limit;
unsigned char iterations;

void main(void) {
    value = 0.0f;
    limit = 1.0f;

    while (value < limit) {
        iterations = iterations + 1;
        value = 2.0f;
    }
}
