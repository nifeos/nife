/* Seeded from relibc (MIT, vendor/relibc/LICENSE) at 893a3b9133ac, 2026-10-10 (UTC), for milestone 835 (vendor/README.md). */
/* Spec:
 * The <stdnoreturn.h> header shall define the macro noreturn which shall
 * expand to _Noreturn */

#ifndef _STDNORETURN_H
#define _STDNORETURN_H

#ifndef __cplusplus
/* Borrowed from musl */
#if __STDC_VERSION__ >= 201112L
#elif defined(__GNUC__)
    #define _Noreturn __attribute__((__noreturn__))
#else
    #define _Noreturn
#endif
#define noreturn _Noreturn
#endif

#endif
