/* A small libc for building CUI to wasm32 with plain clang (no sysroot).
 * Local preview tool only: just enough for the library, the gallery, the
 * demo apps and the component scenes. */
#include <stddef.h>
#include <stdint.h>
#include <stdarg.h>
#include <stdbool.h>
#include <string.h>
#include <stdlib.h>
#include <stdio.h>
#include <math.h>
#include <time.h>
#include <errno.h>
#include <unistd.h>
#include <ctype.h>

/* ---- host imports (host/runtime.js) ---- */
void js_write(int fd, const char *p, int n) WASM_IMPORT(js_write);
int js_fmt_float(double v, int conv, int prec, int alt, char *buf, int cap) WASM_IMPORT(js_fmt_float);
double js_parse_float(const char *p, int n) WASM_IMPORT(js_parse_float);
double js_date_now(void) WASM_IMPORT(js_date_now);
double js_perf_now(void) WASM_IMPORT(js_perf_now);
int js_tz_offset(double epoch_ms) WASM_IMPORT(js_tz_offset);   /* minutes east of UTC */
_Noreturn void js_abort(const char *msg) WASM_IMPORT(js_abort);

int errno;

/* ---- math: the host's Math is correctly rounded for these ---- */
double js_math_sin(double x) WASM_IMPORT(math_sin);
double sin(double x) { return js_math_sin(x); }
double js_math_cos(double x) WASM_IMPORT(math_cos);
double cos(double x) { return js_math_cos(x); }
double js_math_tan(double x) WASM_IMPORT(math_tan);
double tan(double x) { return js_math_tan(x); }
double js_math_asin(double x) WASM_IMPORT(math_asin);
double asin(double x) { return js_math_asin(x); }
double js_math_acos(double x) WASM_IMPORT(math_acos);
double acos(double x) { return js_math_acos(x); }
double js_math_atan(double x) WASM_IMPORT(math_atan);
double atan(double x) { return js_math_atan(x); }
double js_math_sinh(double x) WASM_IMPORT(math_sinh);
double sinh(double x) { return js_math_sinh(x); }
double js_math_cosh(double x) WASM_IMPORT(math_cosh);
double cosh(double x) { return js_math_cosh(x); }
double js_math_tanh(double x) WASM_IMPORT(math_tanh);
double tanh(double x) { return js_math_tanh(x); }
double js_math_exp(double x) WASM_IMPORT(math_exp);
double exp(double x) { return js_math_exp(x); }
double js_math_expm1(double x) WASM_IMPORT(math_expm1);
double expm1(double x) { return js_math_expm1(x); }
double js_math_log(double x) WASM_IMPORT(math_log);
double log(double x) { return js_math_log(x); }
double js_math_log10(double x) WASM_IMPORT(math_log10);
double log10(double x) { return js_math_log10(x); }
double js_math_log2(double x) WASM_IMPORT(math_log2);
double log2(double x) { return js_math_log2(x); }
double js_math_log1p(double x) WASM_IMPORT(math_log1p);
double log1p(double x) { return js_math_log1p(x); }
double js_math_cbrt(double x) WASM_IMPORT(math_cbrt);
double cbrt(double x) { return js_math_cbrt(x); }
double js_math_atan2(double x, double y) WASM_IMPORT(math_atan2);
double atan2(double x, double y) { return js_math_atan2(x, y); }
double js_math_pow(double x, double y) WASM_IMPORT(math_pow);
double pow(double x, double y) { return js_math_pow(x, y); }
double js_math_hypot(double x, double y) WASM_IMPORT(math_hypot);
double hypot(double x, double y) { return js_math_hypot(x, y); }
double js_math_fmod(double x, double y) WASM_IMPORT(math_fmod);
double fmod(double x, double y) { return js_math_fmod(x, y); }

/* float variants (clang may also emit these as libcalls) */
float sinf(float x) { return (float)sin(x); }
float cosf(float x) { return (float)cos(x); }
float tanf(float x) { return (float)tan(x); }
float asinf(float x) { return (float)asin(x); }
float acosf(float x) { return (float)acos(x); }
float atanf(float x) { return (float)atan(x); }
float atan2f(float y, float x) { return (float)atan2(y, x); }
float sinhf(float x) { return (float)sinh(x); }
float coshf(float x) { return (float)cosh(x); }
float tanhf(float x) { return (float)tanh(x); }
float expf(float x) { return (float)exp(x); }
float exp2f(float x) { return (float)pow(2.0, x); }
float expm1f(float x) { return (float)expm1(x); }
float logf(float x) { return (float)log(x); }
float log10f(float x) { return (float)log10(x); }
float log2f(float x) { return (float)log2(x); }
float log1pf(float x) { return (float)log1p(x); }
float powf(float x, float y) { return (float)pow(x, y); }
float cbrtf(float x) { return (float)cbrt(x); }
float hypotf(float x, float y) { return (float)hypot(x, y); }
float fmodf(float x, float y) { return (float)fmod(x, y); }

/* ---- memory ------------------------------------------------------------ *
 * Segregated free lists, one per power of two. Each block carries a 16 byte
 * header with its class so free and realloc know the size. Simple, fast and
 * good enough for a UI that reuses the same buffers every frame. */
extern unsigned char __heap_base;
static uintptr_t heap_top, heap_end;
typedef struct block { struct block *next; } block;
static block *free_lists[40];
#define HDR 16

static int size_class(size_t n) {
    size_t need = n + HDR;
    int c = 5;                                   /* 32 bytes minimum */
    while (((size_t)1 << c) < need) c++;
    return c;
}

static void *heap_take(size_t bytes) {
    uintptr_t p = (heap_top + 15) & ~(uintptr_t)15;
    if (p + bytes > heap_end) {
        size_t want = p + bytes - heap_end;
        size_t pages = (want + 65535) / 65536;
        size_t have = __builtin_wasm_memory_size(0);
        if (pages < have / 4) pages = have / 4;   /* grow in sensible steps */
        if (__builtin_wasm_memory_grow(0, pages) == (size_t)-1) {
            pages = (want + 65535) / 65536;
            if (__builtin_wasm_memory_grow(0, pages) == (size_t)-1) return NULL;
        }
        heap_end = __builtin_wasm_memory_size(0) * 65536;
    }
    heap_top = p + bytes;
    return (void *)p;
}

void *malloc(size_t n) {
    if (!heap_top) { heap_top = (uintptr_t)&__heap_base; heap_end = __builtin_wasm_memory_size(0) * 65536; }
    int c = size_class(n);
    if (c >= 32) return NULL;
    unsigned char *b;
    if (free_lists[c]) { b = (unsigned char *)free_lists[c]; free_lists[c] = free_lists[c]->next; }
    else if (!(b = heap_take((size_t)1 << c))) { errno = ENOMEM; return NULL; }
    *(int *)b = c;
    return b + HDR;
}

void free(void *p) {
    if (!p) return;
    unsigned char *b = (unsigned char *)p - HDR;
    int c = *(int *)b;
    ((block *)b)->next = free_lists[c];
    free_lists[c] = (block *)b;
}

void *calloc(size_t c, size_t n) {
    if (n && c > SIZE_MAX / n) return NULL;
    void *p = malloc(c * n);
    if (p) memset(p, 0, c * n);
    return p;
}

void *realloc(void *p, size_t n) {
    if (!p) return malloc(n);
    if (!n) { free(p); return NULL; }
    int c = *(int *)((unsigned char *)p - HDR);
    size_t cap = ((size_t)1 << c) - HDR;
    if (n <= cap) return p;
    void *q = malloc(n);
    if (!q) return NULL;
    memcpy(q, p, cap);
    free(p);
    return q;
}

/* ---- memory and strings ---- */
void *memcpy(void *restrict d, const void *restrict s, size_t n) {
#if defined(__wasm_bulk_memory__)
    __builtin_memcpy(d, s, n); return d;
#else
    unsigned char *dp = d; const unsigned char *sp = s;
    while (n--) *dp++ = *sp++;
    return d;
#endif
}
void *memmove(void *d, const void *s, size_t n) {
    unsigned char *dp = d; const unsigned char *sp = s;
    if (dp == sp || !n) return d;
    if (dp < sp) { while (n--) *dp++ = *sp++; }
    else { dp += n; sp += n; while (n--) *--dp = *--sp; }
    return d;
}
void *memset(void *d, int c, size_t n) {
    unsigned char *dp = d;
    while (n--) *dp++ = (unsigned char)c;
    return d;
}
int memcmp(const void *a, const void *b, size_t n) {
    const unsigned char *x = a, *y = b;
    for (; n; n--, x++, y++) if (*x != *y) return *x - *y;
    return 0;
}
void *memchr(const void *s, int c, size_t n) {
    const unsigned char *p = s;
    for (; n; n--, p++) if (*p == (unsigned char)c) return (void *)p;
    return NULL;
}
size_t strlen(const char *s) { const char *p = s; while (*p) p++; return (size_t)(p - s); }
size_t strnlen(const char *s, size_t n) { size_t i = 0; while (i < n && s[i]) i++; return i; }
int strcmp(const char *a, const char *b) { while (*a && *a == *b) { a++; b++; } return (unsigned char)*a - (unsigned char)*b; }
int strncmp(const char *a, const char *b, size_t n) {
    for (; n; n--, a++, b++) { if (*a != *b) return (unsigned char)*a - (unsigned char)*b; if (!*a) return 0; }
    return 0;
}
int strcasecmp(const char *a, const char *b) {
    while (*a && tolower((unsigned char)*a) == tolower((unsigned char)*b)) { a++; b++; }
    return tolower((unsigned char)*a) - tolower((unsigned char)*b);
}
int strncasecmp(const char *a, const char *b, size_t n) {
    for (; n; n--, a++, b++) {
        int x = tolower((unsigned char)*a), y = tolower((unsigned char)*b);
        if (x != y) return x - y;
        if (!x) return 0;
    }
    return 0;
}
char *strcpy(char *restrict d, const char *restrict s) { char *r = d; while ((*d++ = *s++)) {} return r; }
char *strncpy(char *restrict d, const char *restrict s, size_t n) {
    size_t i = 0;
    for (; i < n && s[i]; i++) d[i] = s[i];
    for (; i < n; i++) d[i] = 0;
    return d;
}
char *strcat(char *restrict d, const char *restrict s) { strcpy(d + strlen(d), s); return d; }
char *strncat(char *restrict d, const char *restrict s, size_t n) {
    char *e = d + strlen(d);
    while (n-- && *s) *e++ = *s++;
    *e = 0;
    return d;
}
char *strchr(const char *s, int c) {
    for (;; s++) { if (*s == (char)c) return (char *)s; if (!*s) return NULL; }
}
char *strrchr(const char *s, int c) {
    const char *r = NULL;
    for (;; s++) { if (*s == (char)c) r = s; if (!*s) return (char *)r; }
}
char *strstr(const char *h, const char *n) {
    size_t nl = strlen(n);
    if (!nl) return (char *)h;
    for (; *h; h++) if (*h == *n && strncmp(h, n, nl) == 0) return (char *)h;
    return NULL;
}
char *strdup(const char *s) { size_t n = strlen(s) + 1; char *d = malloc(n); if (d) memcpy(d, s, n); return d; }
size_t strspn(const char *s, const char *a) { size_t i = 0; while (s[i] && strchr(a, s[i])) i++; return i; }
size_t strcspn(const char *s, const char *r) { size_t i = 0; while (s[i] && !strchr(r, s[i])) i++; return i; }
char *strerror(int e) { (void)e; return (char *)"error"; }

/* ---- numbers ---- */
static int digit_val(int c) {
    if (c >= '0' && c <= '9') return c - '0';
    if (c >= 'a' && c <= 'z') return c - 'a' + 10;
    if (c >= 'A' && c <= 'Z') return c - 'A' + 10;
    return 99;
}
unsigned long long strtoull(const char *restrict s, char **restrict end, int base) {
    const char *p = s;
    while (isspace((unsigned char)*p)) p++;
    bool neg = false;
    if (*p == '+' || *p == '-') neg = *p++ == '-';
    if ((base == 0 || base == 16) && p[0] == '0' && (p[1] == 'x' || p[1] == 'X') && digit_val(p[2]) < 16) { p += 2; base = 16; }
    else if (base == 0 && p[0] == '0') base = 8;
    else if (base == 0) base = 10;
    unsigned long long v = 0;
    const char *start = p;
    while (digit_val((unsigned char)*p) < base) v = v * (unsigned)base + (unsigned)digit_val((unsigned char)*p++);
    if (end) *end = (char *)(p == start ? s : p);
    return neg ? (unsigned long long)(-(long long)v) : v;
}
long long strtoll(const char *restrict s, char **restrict end, int base) { return (long long)strtoull(s, end, base); }
long strtol(const char *restrict s, char **restrict end, int base) { return (long)strtoull(s, end, base); }
unsigned long strtoul(const char *restrict s, char **restrict end, int base) { return (unsigned long)strtoull(s, end, base); }
int atoi(const char *s) { return (int)strtol(s, NULL, 10); }
long atol(const char *s) { return strtol(s, NULL, 10); }

/* Finds the longest prefix that is a decimal number, then lets the host
 * parse it (correctly rounded). */
double strtod(const char *restrict s, char **restrict end) {
    const char *p = s;
    while (isspace((unsigned char)*p)) p++;
    const char *num = p;
    if (*p == '+' || *p == '-') p++;
    if (strncasecmp(p, "inf", 3) == 0 || strncasecmp(p, "nan", 3) == 0) {
        bool nan = tolower((unsigned char)*p) == 'n';
        p += 3;
        if (!nan && strncasecmp(p, "inity", 5) == 0) p += 5;
        if (end) *end = (char *)p;
        double v = nan ? (double)NAN : (double)INFINITY;
        return *num == '-' ? -v : v;
    }
    const char *digits = p;
    while (isdigit((unsigned char)*p)) p++;
    if (*p == '.') { p++; while (isdigit((unsigned char)*p)) p++; }
    if (p == digits || (p == digits + 1 && *digits == '.')) { if (end) *end = (char *)s; return 0; }
    if (*p == 'e' || *p == 'E') {
        const char *q = p + 1;
        if (*q == '+' || *q == '-') q++;
        if (isdigit((unsigned char)*q)) { while (isdigit((unsigned char)*q)) q++; p = q; }
    }
    if (end) *end = (char *)p;
    return js_parse_float(num, (int)(p - num));
}
float strtof(const char *restrict s, char **restrict end) { return (float)strtod(s, end); }
double atof(const char *s) { return strtod(s, NULL); }

double frexp(double x, int *e) {
    *e = 0;
    if (x == 0 || isnan(x) || isinf(x)) return x;
    union { double d; uint64_t u; } v = { x };
    int ex = (int)((v.u >> 52) & 0x7ff);
    if (!ex) { v.d *= 0x1p64; ex = (int)((v.u >> 52) & 0x7ff) - 64; }
    *e = ex - 1022;
    v.u = (v.u & ~(0x7ffull << 52)) | (1022ull << 52);
    return v.d;
}
float frexpf(float x, int *e) { return (float)frexp(x, e); }
double ldexp(double x, int e) {
    while (e > 1000) { x *= 0x1p1000; e -= 1000; }
    while (e < -1000) { x *= 0x1p-1000; e += 1000; }
    union { double d; uint64_t u; } f = { .u = (uint64_t)(e + 1023) << 52 };
    return x * f.d;
}
float ldexpf(float x, int e) { return (float)ldexp(x, e); }
double modf(double x, double *ip) { double t = trunc(x); *ip = t; return isinf(x) ? copysign(0.0, x) : x - t; }
float modff(float x, float *ip) { float t = truncf(x); *ip = t; return x - t; }

void qsort(void *base, size_t n, size_t size, int (*cmp)(const void *, const void *)) {
    /* insertion sort for small inputs, otherwise a simple heap sort: O(n log n), in place, no recursion */
    unsigned char *a = base, tmp[256];
    unsigned char *t = size <= sizeof tmp ? tmp : malloc(size);
    if (!t) return;
#define EL(i) (a + (size_t)(i) * size)
#define SWAP(i, j) do { memcpy(t, EL(i), size); memcpy(EL(i), EL(j), size); memcpy(EL(j), t, size); } while (0)
    if (n < 16) {
        for (size_t i = 1; i < n; i++)
            for (size_t j = i; j > 0 && cmp(EL(j - 1), EL(j)) > 0; j--) SWAP(j - 1, j);
    } else {
        for (size_t start = n / 2; start-- > 0;) {
            size_t root = start;
            for (;;) { size_t c = 2 * root + 1; if (c >= n) break; if (c + 1 < n && cmp(EL(c), EL(c + 1)) < 0) c++; if (cmp(EL(root), EL(c)) >= 0) break; SWAP(root, c); root = c; }
        }
        for (size_t end = n - 1; end > 0; end--) {
            SWAP(0, end);
            size_t root = 0;
            for (;;) { size_t c = 2 * root + 1; if (c >= end) break; if (c + 1 < end && cmp(EL(c), EL(c + 1)) < 0) c++; if (cmp(EL(root), EL(c)) >= 0) break; SWAP(root, c); root = c; }
        }
    }
#undef SWAP
#undef EL
    if (t != tmp) free(t);
}
void *bsearch(const void *key, const void *base, size_t n, size_t size, int (*cmp)(const void *, const void *)) {
    const unsigned char *a = base;
    size_t lo = 0, hi = n;
    while (lo < hi) {
        size_t mid = (lo + hi) / 2;
        int r = cmp(key, a + mid * size);
        if (!r) return (void *)(a + mid * size);
        if (r < 0) hi = mid; else lo = mid + 1;
    }
    return NULL;
}

static uint64_t rng_state = 0x853c49e6748fea9bull;
int rand(void) { rng_state = rng_state * 6364136223846793005ull + 1442695040888963407ull; return (int)((rng_state >> 33) & RAND_MAX); }
void srand(unsigned seed) { rng_state = seed * 2654435761ull + 1; }
char *getenv(const char *name) { (void)name; return NULL; }
int system(const char *cmd) { (void)cmd; return -1; }
_Noreturn void abort(void) { js_abort("abort()"); }
_Noreturn void exit(int code) { (void)code; js_abort("exit()"); }
_Noreturn void __wasm_assert_fail(const char *expr, const char *file, int line) {
    char buf[512];
    snprintf(buf, sizeof buf, "assertion failed: %s (%s:%d)", expr, file, line);
    js_abort(buf);
}
long sysconf(int name) { (void)name; return 1; }   /* one core: the renderer stays single threaded */
int usleep(unsigned usec) { (void)usec; return 0; }

/* ---- time ---- */
time_t time(time_t *t) { time_t v = (time_t)(js_date_now() / 1000.0); if (t) *t = v; return v; }
clock_t clock(void) { return (clock_t)(js_perf_now() * 1000.0); }
double difftime(time_t a, time_t b) { return (double)(a - b); }
int clock_gettime(clockid_t id, struct timespec *ts) {
    double ms = id == CLOCK_MONOTONIC ? js_perf_now() : js_date_now();
    ts->tv_sec = (time_t)(ms / 1000.0);
    ts->tv_nsec = (long)((ms - (double)ts->tv_sec * 1000.0) * 1e6);
    return 0;
}
int nanosleep(const struct timespec *req, struct timespec *rem) { (void)req; if (rem) rem->tv_sec = rem->tv_nsec = 0; return 0; }

static struct tm *civil(long long secs, struct tm *out) {
    long long days = secs / 86400, rem = secs % 86400;
    if (rem < 0) { rem += 86400; days--; }
    out->tm_hour = (int)(rem / 3600); out->tm_min = (int)(rem % 3600 / 60); out->tm_sec = (int)(rem % 60);
    out->tm_wday = (int)(((days % 7) + 11) % 7);   /* 1970-01-01 was a Thursday */
    /* Howard Hinnant's civil_from_days */
    long long z = days + 719468, era = (z >= 0 ? z : z - 146096) / 146097;
    unsigned doe = (unsigned)(z - era * 146097);
    unsigned yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    long long y = (long long)yoe + era * 400;
    unsigned doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    unsigned mp = (5 * doy + 2) / 153;
    unsigned d = doy - (153 * mp + 2) / 5 + 1;
    unsigned m = mp < 10 ? mp + 3 : mp - 9;
    y += m <= 2;
    out->tm_year = (int)(y - 1900); out->tm_mon = (int)m - 1; out->tm_mday = (int)d;
    static const int cum[12] = { 0, 31, 59, 90, 120, 151, 181, 212, 243, 273, 304, 334 };
    bool leap = (y % 4 == 0 && y % 100 != 0) || y % 400 == 0;
    out->tm_yday = cum[m - 1] + (int)d - 1 + (leap && m > 2);
    out->tm_isdst = 0; out->tm_gmtoff = 0; out->tm_zone = "UTC";
    return out;
}
struct tm *gmtime_r(const time_t *restrict t, struct tm *restrict out) { return civil(*t, out); }
struct tm *localtime_r(const time_t *restrict t, struct tm *restrict out) {
    int off = js_tz_offset((double)*t * 1000.0);
    civil(*t + off * 60LL, out);
    out->tm_gmtoff = off * 60L; out->tm_zone = "local";
    return out;
}
struct tm *gmtime(const time_t *t) { static struct tm tm; return gmtime_r(t, &tm); }
struct tm *localtime(const time_t *t) { static struct tm tm; return localtime_r(t, &tm); }
time_t mktime(struct tm *tm) {
    long long y = tm->tm_year + 1900LL, m = tm->tm_mon + 1;
    y -= m <= 2;
    long long era = (y >= 0 ? y : y - 399) / 400;
    unsigned yoe = (unsigned)(y - era * 400);
    unsigned doy = (unsigned)((153 * (m + (m > 2 ? -3 : 9)) + 2) / 5 + tm->tm_mday - 1);
    unsigned doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    long long days = era * 146097 + doe - 719468;
    long long local = days * 86400 + tm->tm_hour * 3600LL + tm->tm_min * 60LL + tm->tm_sec;
    return (time_t)(local - js_tz_offset((double)local * 1000.0) * 60LL);
}
size_t strftime(char *restrict s, size_t n, const char *restrict f, const struct tm *restrict tm) {
    (void)f; (void)tm;
    if (n) s[0] = 0;
    return 0;
}

/* ---- stdio ---- */
struct FILE { int fd; char buf[512]; int len; };
static FILE f_in = { 0 }, f_out = { 1 }, f_err = { 2 };
FILE *const stdin = &f_in, *const stdout = &f_out, *const stderr = &f_err;

int fflush(FILE *fp) {
    if (fp && fp->len) { js_write(fp->fd, fp->buf, fp->len); fp->len = 0; }
    return 0;
}
static void file_put(FILE *fp, const char *p, size_t n) {
    for (size_t i = 0; i < n; i++) {
        if (fp->len == (int)sizeof fp->buf) fflush(fp);
        fp->buf[fp->len++] = p[i];
        if (p[i] == '\n') fflush(fp);
    }
}
FILE *fopen(const char *restrict path, const char *restrict mode) { (void)path; (void)mode; errno = EINVAL; return NULL; }
int fclose(FILE *fp) { fflush(fp); return 0; }
size_t fread(void *restrict p, size_t s, size_t n, FILE *restrict fp) { (void)p; (void)s; (void)n; (void)fp; return 0; }
size_t fwrite(const void *restrict p, size_t s, size_t n, FILE *restrict fp) {
    if (fp != stdout && fp != stderr) return 0;
    file_put(fp, p, s * n);
    return n;
}
int fseek(FILE *fp, long off, int whence) { (void)fp; (void)off; (void)whence; return -1; }
long ftell(FILE *fp) { (void)fp; return -1; }
int feof(FILE *fp) { (void)fp; return 1; }
int ferror(FILE *fp) { (void)fp; return 0; }
int fgetc(FILE *fp) { (void)fp; return EOF; }
int getc(FILE *fp) { (void)fp; return EOF; }
int ungetc(int c, FILE *fp) { (void)c; (void)fp; return EOF; }
char *fgets(char *restrict s, int n, FILE *restrict fp) { (void)s; (void)n; (void)fp; return NULL; }
int remove(const char *path) { (void)path; return -1; }
int fputs(const char *restrict s, FILE *restrict fp) { file_put(fp, s, strlen(s)); return 0; }
int fputc(int c, FILE *fp) { char ch = (char)c; file_put(fp, &ch, 1); return c; }
int putc(int c, FILE *fp) { return fputc(c, fp); }
int putchar(int c) { return fputc(c, stdout); }
int puts(const char *s) { fputs(s, stdout); fputc('\n', stdout); return 0; }
int sscanf(const char *restrict s, const char *restrict f, ...) { (void)s; (void)f; return 0; }

/* ---- printf ---- */
typedef struct { char *buf; size_t cap, len; FILE *fp; } sink;
static void out(sink *k, const char *p, size_t n) {
    if (k->fp) { file_put(k->fp, p, n); k->len += n; return; }
    for (size_t i = 0; i < n; i++, k->len++) if (k->len + 1 < k->cap) k->buf[k->len] = p[i];
}
static void pad(sink *k, char c, int n) { while (n-- > 0) out(k, &c, 1); }

/* Emits prefix + body with width, '-' and '0' handling. zeros: extra leading zeros (integer precision). */
static void emit(sink *k, const char *prefix, int plen, const char *body, int blen, int zeros, int width, bool left, bool zero_pad) {
    int total = plen + zeros + blen;
    if (!left && !zero_pad) pad(k, ' ', width - total);
    out(k, prefix, (size_t)plen);
    if (!left && zero_pad) pad(k, '0', width - total);
    pad(k, '0', zeros);
    out(k, body, (size_t)blen);
    if (left) pad(k, ' ', width - total);
}

static int vformat(sink *k, const char *f, va_list ap) {
    for (; *f; f++) {
        if (*f != '%') { const char *s = f; while (f[1] && f[1] != '%') f++; out(k, s, (size_t)(f - s + 1)); continue; }
        f++;
        bool left = false, plus = false, space = false, alt = false, zero = false;
        for (;; f++) {
            if (*f == '-') left = true; else if (*f == '+') plus = true; else if (*f == ' ') space = true;
            else if (*f == '#') alt = true; else if (*f == '0') zero = true; else break;
        }
        int width = 0, prec = -1;
        if (*f == '*') { width = va_arg(ap, int); if (width < 0) { left = true; width = -width; } f++; }
        else while (isdigit((unsigned char)*f)) width = width * 10 + (*f++ - '0');
        if (*f == '.') {
            f++; prec = 0;
            if (*f == '*') { prec = va_arg(ap, int); f++; }
            else while (isdigit((unsigned char)*f)) prec = prec * 10 + (*f++ - '0');
        }
        int len = 0;   /* 0 int, 1 long, 2 long long, -1 short, -2 char, 3 size_t */
        for (;; f++) {
            if (*f == 'l') len = len == 1 ? 2 : 1;
            else if (*f == 'h') len = len == -1 ? -2 : -1;
            else if (*f == 'z' || *f == 't') len = 3;
            else if (*f == 'j') len = 2;
            else if (*f == 'L') len = 2;
            else break;
        }
        char conv = *f, tmp[400];
        if (!conv) break;
        switch (conv) {
        case '%': out(k, "%", 1); break;
        case 'c': { char ch = (char)va_arg(ap, int); emit(k, "", 0, &ch, 1, 0, width, left, false); break; }
        case 's': {
            const char *s = va_arg(ap, const char *);
            if (!s) s = "(null)";
            int n = (int)(prec >= 0 ? strnlen(s, (size_t)prec) : strlen(s));
            emit(k, "", 0, s, n, 0, width, left, false);
            break;
        }
        case 'd': case 'i': case 'u': case 'x': case 'X': case 'o': case 'p': {
            unsigned long long v; bool neg = false;
            if (conv == 'p') { v = (uintptr_t)va_arg(ap, void *); alt = true; }
            else if (conv == 'd' || conv == 'i') {
                long long sv = len == 2 ? va_arg(ap, long long) : len == 1 ? va_arg(ap, long) : len == 3 ? (long long)va_arg(ap, ptrdiff_t) : va_arg(ap, int);
                if (len == -1) sv = (short)sv; else if (len == -2) sv = (signed char)sv;
                neg = sv < 0; v = neg ? 0ull - (unsigned long long)sv : (unsigned long long)sv;
            } else {
                v = len == 2 ? va_arg(ap, unsigned long long) : len == 1 ? va_arg(ap, unsigned long) : len == 3 ? va_arg(ap, size_t) : va_arg(ap, unsigned);
                if (len == -1) v = (unsigned short)v; else if (len == -2) v = (unsigned char)v;
            }
            unsigned base = conv == 'o' ? 8 : (conv == 'x' || conv == 'X' || conv == 'p') ? 16 : 10;
            const char *dig = conv == 'X' ? "0123456789ABCDEF" : "0123456789abcdef";
            char *e = tmp + sizeof tmp, *p = e;
            while (v) { *--p = dig[v % base]; v /= base; }
            if (p == e && prec != 0) *--p = '0';
            int blen = (int)(e - p);
            char prefix[3]; int plen = 0;
            if (neg) prefix[plen++] = '-'; else if (plus && (conv == 'd' || conv == 'i')) prefix[plen++] = '+';
            else if (space && (conv == 'd' || conv == 'i')) prefix[plen++] = ' ';
            if (alt && base == 16 && blen && !(blen == 1 && *p == '0' && conv != 'p')) { prefix[plen++] = '0'; prefix[plen++] = conv == 'X' ? 'X' : 'x'; }
            int zeros = prec > blen ? prec - blen : 0;
            if (alt && base == 8 && !zeros && *p != '0') zeros = 1;
            emit(k, prefix, plen, p, blen, zeros, width, left, zero && prec < 0 && !left);
            break;
        }
        case 'f': case 'F': case 'e': case 'E': case 'g': case 'G': case 'a': case 'A': {
            double v = va_arg(ap, double);
            if (prec < 0) prec = 6;
            if (prec > 300) prec = 300;
            bool neg = signbit(v);
            char prefix[1]; int plen = 0;
            if (neg) prefix[plen++] = '-'; else if (plus) prefix[plen++] = '+'; else if (space) prefix[plen++] = ' ';
            int n = js_fmt_float(fabs(v), conv, prec, alt, tmp, (int)sizeof tmp);
            emit(k, prefix, plen, tmp, n, 0, width, left, zero && !left && isfinite(v));
            break;
        }
        case 'n': { int *np = va_arg(ap, int *); *np = (int)k->len; break; }
        default: out(k, f - 1, 2); break;
        }
    }
    return (int)k->len;
}

int vsnprintf(char *restrict b, size_t n, const char *restrict f, va_list ap) {
    sink k = { b, n, 0, NULL };
    int r = vformat(&k, f, ap);
    if (n) b[k.len < n ? k.len : n - 1] = 0;
    return r;
}
int vsprintf(char *restrict b, const char *restrict f, va_list ap) { return vsnprintf(b, (size_t)-1 >> 1, f, ap); }
int vfprintf(FILE *restrict fp, const char *restrict f, va_list ap) { sink k = { NULL, 0, 0, fp }; return vformat(&k, f, ap); }
int vprintf(const char *restrict f, va_list ap) { return vfprintf(stdout, f, ap); }
int snprintf(char *restrict b, size_t n, const char *restrict f, ...) { va_list ap; va_start(ap, f); int r = vsnprintf(b, n, f, ap); va_end(ap); return r; }
int sprintf(char *restrict b, const char *restrict f, ...) { va_list ap; va_start(ap, f); int r = vsprintf(b, f, ap); va_end(ap); return r; }
int fprintf(FILE *restrict fp, const char *restrict f, ...) { va_list ap; va_start(ap, f); int r = vfprintf(fp, f, ap); va_end(ap); return r; }
int printf(const char *restrict f, ...) { va_list ap; va_start(ap, f); int r = vfprintf(stdout, f, ap); va_end(ap); return r; }
