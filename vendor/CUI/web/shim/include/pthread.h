#ifndef _PTHREAD_H
#define _PTHREAD_H
/* No threads on this target: creation always fails, so callers run the work inline. */
typedef unsigned long pthread_t;
typedef struct { int unused; } pthread_attr_t;
static inline int pthread_create(pthread_t *t, const pthread_attr_t *a, void *(*fn)(void *), void *arg) { (void)t; (void)a; (void)fn; (void)arg; return 11; }
static inline int pthread_join(pthread_t t, void **ret) { (void)t; (void)ret; return 3; }
#endif
