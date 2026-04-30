#include <pic16/pic16f628a.h>

enum State {
    STATE_IDLE,
    STATE_RUN,
    STATE_ERROR
};

void main(void) {
    enum State state = STATE_IDLE;

    TRISB = 0x00;

    while (1) {
        switch (state) {
            case STATE_IDLE:
                PORTB = 0x00;
                state = STATE_RUN;
                break;

            case STATE_RUN:
                PORTB = 0x01;
                state = STATE_ERROR;
                break;

            default:
                PORTB = 0xFF;
                break;
        }
    }
}
