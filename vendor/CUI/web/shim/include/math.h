#ifndef _MATH_H
#define _MATH_H
/* Math for wasm32. Rounding, sqrt, abs and min/max are single wasm
 * instructions; the transcendental functions call the host's Math object
 * (shim/libc.c, host/runtime.js). */
#include "_wasm_libc.h"

#define M_E        2.71828182845904523536
#define M_LN2      0.69314718055994530942
#define M_LN10     2.30258509299404568402
#define M_PI       3.14159265358979323846
#define M_PI_2     1.57079632679489661923
#define M_PI_4     0.78539816339744830962
#define M_1_PI     0.31830988618379067154
#define M_2_PI     0.63661977236758134308
#define M_SQRT2    1.41421356237309504880
#define M_SQRT1_2  0.70710678118654752440
#define HUGE_VAL   __builtin_huge_val()
#define HUGE_VALF  __builtin_huge_valf()
#define INFINITY   __builtin_inff()
#define NAN        __builtin_nanf("")
#define FP_NAN 0
#define FP_INFINITE 1
#define FP_ZERO 2
#define FP_SUBNORMAL 3
#define FP_NORMAL 4
#define isnan(x)    __builtin_isnan(x)
#define isinf(x)    __builtin_isinf(x)
#define isfinite(x) __builtin_isfinite(x)
#define isnormal(x) __builtin_isnormal(x)
#define signbit(x)  __builtin_signbit(x)
#define fpclassify(x) __builtin_fpclassify(FP_NAN, FP_INFINITE, FP_NORMAL, FP_SUBNORMAL, FP_ZERO, x)

static inline double fabs(double x) { return __builtin_fabs(x); }
static inline float fabsf(float x) { return __builtin_fabsf(x); }
static inline double sqrt(double x) { return __builtin_sqrt(x); }
static inline float sqrtf(float x) { return __builtin_sqrtf(x); }
static inline double floor(double x) { return __builtin_floor(x); }
static inline float floorf(float x) { return __builtin_floorf(x); }
static inline double ceil(double x) { return __builtin_ceil(x); }
static inline float ceilf(float x) { return __builtin_ceilf(x); }
static inline double trunc(double x) { return __builtin_trunc(x); }
static inline float truncf(float x) { return __builtin_truncf(x); }
static inline double rint(double x) { return __builtin_rint(x); }
static inline float rintf(float x) { return __builtin_rintf(x); }
static inline double nearbyint(double x) { return __builtin_nearbyint(x); }
static inline float nearbyintf(float x) { return __builtin_nearbyintf(x); }
static inline double copysign(double a, double b) { return __builtin_copysign(a, b); }
static inline float copysignf(float a, float b) { return __builtin_copysignf(a, b); }
static inline double fmin(double a, double b) { return a < b || b != b ? a : b; }
static inline double fmax(double a, double b) { return a > b || b != b ? a : b; }
static inline float fminf(float a, float b) { return a < b || b != b ? a : b; }
static inline float fmaxf(float a, float b) { return a > b || b != b ? a : b; }
/* round half away from zero; x - trunc(x) is exact for doubles */
static inline double round(double x) { double t = __builtin_trunc(x); if (__builtin_fabs(x - t) >= 0.5) t += __builtin_copysign(1.0, x); return t; }
static inline float roundf(float x) { float t = __builtin_truncf(x); if (__builtin_fabsf(x - t) >= 0.5f) t += __builtin_copysignf(1.0f, x); return t; }
static inline long lround(double x) { return (long)round(x); }
static inline long lroundf(float x) { return (long)roundf(x); }
static inline long long llround(double x) { return (long long)round(x); }
static inline long lrint(double x) { return (long)__builtin_rint(x); }
static inline long lrintf(float x) { return (long)__builtin_rintf(x); }

double sin(double x);
double cos(double x);
double tan(double x);
double asin(double x);
double acos(double x);
double atan(double x);
double atan2(double y, double x);
double sinh(double x);
double cosh(double x);
double tanh(double x);
double exp(double x);
double expm1(double x);
double log(double x);
double log10(double x);
double log2(double x);
double log1p(double x);
double pow(double x, double y);
double cbrt(double x);
double hypot(double x, double y);
double fmod(double x, double y);

float sinf(float x);
float cosf(float x);
float tanf(float x);
float asinf(float x);
float acosf(float x);
float atanf(float x);
float atan2f(float y, float x);
float sinhf(float x);
float coshf(float x);
float tanhf(float x);
float expf(float x);
float exp2f(float x);
static inline double exp2(double x) { return pow(2.0, x); }
float expm1f(float x);
float logf(float x);
float log10f(float x);
float log2f(float x);
float log1pf(float x);
float powf(float x, float y);
float cbrtf(float x);
float hypotf(float x, float y);
float fmodf(float x, float y);

double frexp(double x, int *e);
float frexpf(float x, int *e);
double ldexp(double x, int e);
float ldexpf(float x, int e);
double modf(double x, double *ip);
float modff(float x, float *ip);
static inline double scalbn(double x, int e) { return ldexp(x, e); }
static inline float scalbnf(float x, int e) { return ldexpf(x, e); }

#endif
