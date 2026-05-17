float temperature;
float threshold;
unsigned char alarm;
unsigned long total;
unsigned long rem;

void main(void) {
    temperature = 31.5f;
    threshold = 30.0f;
    alarm = temperature > threshold;

    total = 100000UL / 300UL;
    rem = 100000UL % 300UL;
}
