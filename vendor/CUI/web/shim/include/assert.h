#undef assert
#ifdef NDEBUG
#define assert(x) ((void)0)
#else
_Noreturn void __wasm_assert_fail(const char *expr, const char *file, int line);
#define assert(x) ((x) ? (void)0 : __wasm_assert_fail(#x, __FILE__, __LINE__))
#endif
#ifndef static_assert
#define static_assert _Static_assert
#endif
