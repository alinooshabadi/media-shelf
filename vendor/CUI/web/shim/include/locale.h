#ifndef _LOCALE_H
#define _LOCALE_H
#define LC_ALL 6
#define LC_CTYPE 0
#define LC_NUMERIC 1
static inline char *setlocale(int c, const char *l) { (void)c; (void)l; return (char *)"C"; }
#endif
