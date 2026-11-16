/* Tiny libc for CUI on wasm32 (local preview tool only). */
#ifndef WASM_LIBC_H
#define WASM_LIBC_H
#include <stddef.h>
#include <stdint.h>
#include <stdarg.h>
#define WASM_IMPORT(name) __attribute__((import_module("env"), import_name(#name)))
#ifndef NULL
#define NULL ((void *)0)
#endif
#endif
