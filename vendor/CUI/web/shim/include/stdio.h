#ifndef _STDIO_H
#define _STDIO_H
#include "_wasm_libc.h"
typedef struct FILE FILE;
extern FILE *const stdin, *const stdout, *const stderr;
#define stdin stdin
#define stdout stdout
#define stderr stderr
#define EOF (-1)
#define SEEK_SET 0
#define SEEK_CUR 1
#define SEEK_END 2
#define BUFSIZ 1024
int printf(const char *restrict f, ...);
int fprintf(FILE *restrict fp, const char *restrict f, ...);
int sprintf(char *restrict b, const char *restrict f, ...);
int snprintf(char *restrict b, size_t n, const char *restrict f, ...);
int vprintf(const char *restrict f, va_list ap);
int vfprintf(FILE *restrict fp, const char *restrict f, va_list ap);
int vsprintf(char *restrict b, const char *restrict f, va_list ap);
int vsnprintf(char *restrict b, size_t n, const char *restrict f, va_list ap);
int puts(const char *s);
int fputs(const char *restrict s, FILE *restrict fp);
int fputc(int c, FILE *fp);
int putc(int c, FILE *fp);
int putchar(int c);
int fflush(FILE *fp);
FILE *fopen(const char *restrict path, const char *restrict mode);
int fclose(FILE *fp);
size_t fread(void *restrict p, size_t s, size_t n, FILE *restrict fp);
size_t fwrite(const void *restrict p, size_t s, size_t n, FILE *restrict fp);
int fseek(FILE *fp, long off, int whence);
long ftell(FILE *fp);
int feof(FILE *fp);
int ferror(FILE *fp);
int fgetc(FILE *fp);
int getc(FILE *fp);
int ungetc(int c, FILE *fp);
char *fgets(char *restrict s, int n, FILE *restrict fp);
int sscanf(const char *restrict s, const char *restrict f, ...);
int remove(const char *path);
#endif
