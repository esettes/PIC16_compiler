float gain = 1.5f;
float samples[] = {
    1.0f,
    2.0f
};

float result;

void main(void) {
    static float offset = 2.0f;
    result = samples[1] + offset;
}
