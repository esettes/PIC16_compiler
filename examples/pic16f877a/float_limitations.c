float input;
float output;

void main(void) {
    input = 1.5f;
    output = input * 2.0f;
}

/* Phase 28 limitations:
 * - no double
 * - no math library
 * - no float ROM tables
 * - dynamic long/unsigned long <-> float casts are rejected
 */
