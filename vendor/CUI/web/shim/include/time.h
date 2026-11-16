#ifndef _TIME_H
#define _TIME_H
#include "_wasm_libc.h"
typedef long long time_t;
typedef long clock_t;
#define CLOCKS_PER_SEC 1000000L
struct tm { int tm_sec, tm_min, tm_hour, tm_mday, tm_mon, tm_year, tm_wday, tm_yday, tm_isdst; long tm_gmtoff; const char *tm_zone; };
struct timespec { time_t tv_sec; long tv_nsec; };
typedef int clockid_t;
#define CLOCK_REALTIME 0
#define CLOCK_MONOTONIC 1
time_t time(time_t *t);
clock_t clock(void);
double difftime(time_t a, time_t b);
struct tm *localtime_r(const time_t *restrict t, struct tm *restrict out);
struct tm *localtime(const time_t *t);
struct tm *gmtime_r(const time_t *restrict t, struct tm *restrict out);
struct tm *gmtime(const time_t *t);
time_t mktime(struct tm *tm);
int clock_gettime(clockid_t id, struct timespec *ts);
int nanosleep(const struct timespec *req, struct timespec *rem);
size_t strftime(char *restrict s, size_t n, const char *restrict f, const struct tm *restrict tm);
#endif
