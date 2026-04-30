#include <pic16/pic16f628a.h>

struct Point {
    unsigned char x;
    unsigned char y;
};

struct Point anchor = {1, 2};

unsigned char point_sum(struct Point *point) {
    return point->x + point->y;
}

void main(void) {
    struct Point local = {3, 4};
    struct Point *cursor = &local;

    anchor.x = cursor->x;

    TRISB = 0x00;
    PORTB = point_sum(&anchor) + local.y;
}
