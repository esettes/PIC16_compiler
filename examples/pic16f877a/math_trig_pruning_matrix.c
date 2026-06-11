#include <math.h>

/*
 * Phase 50 pruning matrix.
 *
 * Constant folded trig must not emit runtime helpers/tables.
 */

float folded_s = sinf(0.0f);
float folded_c = cosf(0.0f);
float folded_t = tanf(0.78539816f);

void main(void) {
}
