float low;
float high;
unsigned char result;

void main(void) {
    low = -3.0f;
    high = -2.0f;

    if (low < high) {
        result = 1;
    } else {
        result = 0;
    }
}
