#include <pic16/pic16f877a.h>

struct Pair {
    unsigned char lo;
    unsigned char hi;
};

void copy_pair(struct Pair *dst, struct Pair *src) {
    *dst = *src;
}

void main(void) {
    struct Pair left = {1, 2};
    struct Pair right = {3, 4};

    ADCON1 = 0x06;
    TRISB = 0x00;
    copy_pair(&left, &right);
    PORTB = left.hi;
}
