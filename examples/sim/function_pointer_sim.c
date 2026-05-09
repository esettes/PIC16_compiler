/* SPDX-License-Identifier: GPL-3.0-or-later */

unsigned char result;

unsigned char twice(unsigned char value) {
    return value * 2;
}

void main(void) {
    unsigned char (*fn)(unsigned char);
    fn = twice;
    result = fn(21);
}
