/* What each wasm module provides to the shared host (web_host.c).
 * The scenes module serves many small component scenes; an app module serves
 * one demo app. */
#ifndef WEB_HOST_H
#define WEB_HOST_H

#include <cui/cui.h>

int web_content_count(void);
const char *web_content_name(int i);
float web_content_width(int i);
float web_content_height(int i);
int web_content_accent(int i);          /* cui_accent, or -1 for the default */
int web_content_theme(int i);           /* 0 light, 1 dark, -1 either */
int web_content_warmup(int i);          /* scripted frames before going live */
/* f < 0: scripted input before warm-up frame (-f - 1). f >= 0: build the UI. */
void web_content_input(int i, cui_ctx *ui, int f);
void web_content_build(int i, cui_ctx *ui, int f);

#endif
