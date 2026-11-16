#ifndef _STDLIB_H
#define _STDLIB_H
#include "_wasm_libc.h"
#define EXIT_SUCCESS 0
#define EXIT_FAILURE 1
#define RAND_MAX 0x7fffffff
void *malloc(size_t n);
void *calloc(size_t c, size_t n);
void *realloc(void *p, size_t n);
void free(void *p);
_Noreturn void abort(void);
_Noreturn void exit(int code);
int atoi(const char *s);
long atol(const char *s);
double atof(const char *s);
double strtod(const char *restrict s, char **restrict end);
float strtof(const char *restrict s, char **restrict end);
long strtol(const char *restrict s, char **restrict end, int base);
unsigned long strtoul(const char *restrict s, char **restrict end, int base);
long long strtoll(const char *restrict s, char **restrict end, int base);
unsigned long long strtoull(const char *restrict s, char **restrict end, int base);
void qsort(void *base, size_t n, size_t size, int (*cmp)(const void *, const void *));
void *bsearch(const void *key, const void *base, size_t n, size_t size, int (*cmp)(const void *, const void *));
int rand(void);
void srand(unsigned seed);
char *getenv(const char *name);
int system(const char *cmd);
static inline int abs(int x) { return x < 0 ? -x : x; }
static inline long labs(long x) { return x < 0 ? -x : x; }
static inline long long llabs(long long x) { return x < 0 ? -x : x; }
#endif
