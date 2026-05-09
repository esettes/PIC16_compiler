unsigned long a = 1234UL;
unsigned long b = 17UL;
unsigned long c = 100000UL;
unsigned long result;

void main(void) {
    result = (a * b) + (c / 25UL) + (c % 97UL);
}
