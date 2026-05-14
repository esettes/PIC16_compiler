const __rom float gains[] = {
    0.5f,
    1.0f,
    1.5f,
    2.0f
};

unsigned char gain_index;
float gain;

void main(void) {
    gain_index = 2;
    gain = gains[gain_index];
}
