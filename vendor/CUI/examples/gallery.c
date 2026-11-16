/* The CUI component gallery app. */
#include <cui/cui_app.h>
#include "gallery_ui.h"

static void frame(cui_ctx *ui, void *user) { gallery_frame(ui, user); }

int main(void) {
    static gallery g;
    gallery_init(&g);
    return cui_app_run(&(cui_app_desc){ .title = "CUI Gallery", .width = 1240, .height = 820, .min_width = 720, .min_height = 480 }, frame, &g);
}
