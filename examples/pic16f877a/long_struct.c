struct Counter {
    unsigned long value;
};

struct Counter counter = { 100000UL };
unsigned long result;

void main(void) {
    counter.value = counter.value + 250UL;
    result = counter.value;
}
