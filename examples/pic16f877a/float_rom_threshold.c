const __rom float thresholds[] = {
    28.0f,
    30.0f
};

float temperature;
float threshold;
unsigned char alarm;

void main(void) {
    temperature = 31.5f;
    threshold = thresholds[1];
    if (temperature > threshold) {
        alarm = 1;
    } else {
        alarm = 0;
    }
}
