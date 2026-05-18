__ufixed16_16 ua;
__ufixed16_16 ub;
__ufixed16_16 ur;

__fixed16_16 sa;
__fixed16_16 sb;
__fixed16_16 sr;

void main(void) {
    ua = 5.0uq16_16;
    ub = 2.0uq16_16;
    ur = ua / ub;

    sa = -3.0q16_16;
    sb = 2.0q16_16;
    sr = sa / sb;
}
