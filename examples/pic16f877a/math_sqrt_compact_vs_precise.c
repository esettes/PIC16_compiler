// SPDX-License-Identifier: GPL-3.0-or-later
//
// Build this file twice to compare profile cost and accuracy reporting:
//   picc --target pic16f877a --math-profile compact --size --memory-report ...
//   picc --target pic16f877a --math-profile precise --size --memory-report ...

#include <math.h>

float input;
float result;

void main(void) {
    input = 5.0f;
    result = sqrtf(input);
}
