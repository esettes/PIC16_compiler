float temperature;
float threshold;
unsigned char alarm;

void main(void) {
    temperature = 31.5f;
    threshold = 30.0f;
    if (temperature > threshold) {
        alarm = 1;
    } else {
        alarm = 0;
    }
}
