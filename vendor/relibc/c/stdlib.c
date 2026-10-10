/*
 * Seeded from relibc (MIT, c_library/LICENSE-relibc) src/c/stdlib.c at 893a3b9133ac2fb3089f71b02d5b61d145d97968,
 * unchanged, by milestone 835 on 2026-10-10 (UTC). C because `long double` is a C type: 128-bit IEEE
 * on aarch64 and riscv64 and x87 extended precision on x86_64, which Rust has no stable name for.
 * No syscall here or anywhere in this library (§31 rule 1 as amended by §265).
 */
double strtod(const char *nptr, char **endptr);

long double strtold(const char *nptr, char **endptr) {
    return (long double)strtod(nptr, endptr);
}

double relibc_ldtod(const long double* val) {
    return (double)(*val);
}

void relibc_dtold(double val, long double* out) {
    *out = (long double)val;
}
