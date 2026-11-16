/* cui_app for the browser: a demo app's main() calls cui_app_run, which only
 * records the frame callback. The page then drives it through web_host.c.
 * Each example is built with -Dmain=web_app_main. */
#include <cui/cui_app.h>
#include <string.h>
#include "web_host.h"

#define EXPORT(name) __attribute__((export_name(#name)))

static cui_app_desc app_desc;
static void (*app_frame)(cui_ctx *ui, void *user);
static void *app_user;

int web_app_main(void);
EXPORT(web_boot) int web_boot(void) { return web_app_main(); }

int cui_app_run(const cui_app_desc *desc, void (*frame)(cui_ctx *ui, void *user), void *user) {
    if (desc) app_desc = *desc;
    app_frame = frame;
    app_user = user;
    return 0;
}

int web_content_count(void) { return app_frame ? 1 : 0; }
const char *web_content_name(int i) { (void)i; return app_desc.title ? app_desc.title : "CUI"; }
float web_content_width(int i) { (void)i; return app_desc.width ? (float)app_desc.width : 1080.0f; }
float web_content_height(int i) { (void)i; return app_desc.height ? (float)app_desc.height : 720.0f; }
int web_content_accent(int i) { (void)i; return (int)app_desc.accent; }
int web_content_theme(int i) { (void)i; return app_desc.theme == CUI_THEME_DARK ? 1 : 0; }
int web_content_warmup(int i) { (void)i; return 0; }
void web_content_input(int i, cui_ctx *ui, int f) { (void)i; (void)ui; (void)f; }
void web_content_build(int i, cui_ctx *ui, int f) {
    (void)i; (void)f;
    if (app_frame) app_frame(ui, app_user);
}

/* The rest of the app API, for code that calls it. */
cui_app *cui_app_create(const cui_app_desc *desc) { (void)desc; return NULL; }
void cui_app_destroy(cui_app *app) { (void)app; }
bool cui_app_frame(cui_app *app) { (void)app; return false; }
cui_ctx *cui_app_ui(cui_app *app) { (void)app; return NULL; }
void cui_app_quit(cui_app *app) { (void)app; }
void cui_app_set_title(cui_app *app, const char *title) { (void)app; (void)title; }
void cui_app_set_size(cui_app *app, int width, int height) { (void)app; (void)width; (void)height; }
cui_vec2 cui_app_size(cui_app *app) { (void)app; return (cui_vec2){ web_content_width(0), web_content_height(0) }; }
bool cui_app_is_software(cui_app *app) { (void)app; return true; }
void *cui_app_native_window(cui_app *app) { (void)app; return NULL; }
const char *cui_app_backend(cui_app *app) { (void)app; return "web"; }
bool cui_app_file_dialog(cui_app *app, const cui_file_dialog_opts *opts, char *out, size_t cap) { (void)app; (void)opts; (void)out; (void)cap; return false; }
bool cui_app_open_url(const char *url) { (void)url; return false; }
bool cui_app_screenshot(cui_app *app, void *rgba, int width, int height) { (void)app; (void)rgba; (void)width; (void)height; return false; }
