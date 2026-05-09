/* SPDX-License-Identifier: GPL-3.0-or-later */

const __rom unsigned char table[] = { 4, 8, 15, 16 };
unsigned char result;

void main(void) {
    result = __rom_read8(table, 2);
}
