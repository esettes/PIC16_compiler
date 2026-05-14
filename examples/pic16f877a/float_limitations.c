float input;
float output;

void main(void) {
    input = 1.5f;
    output = input * 2.0f;
}

/* Current float limitations:
 * - no double
 * - no math library
 * - no full IEEE NaN/Inf/subnormal runtime model
 * - no implicit mixed float/integer arithmetic
 */
