/* Renders gallery pages headless with the software renderer and writes PNGs.
 *   cui_screenshots <out-dir> [page] [light|dark] [width] [height] */
#include <cui/cui.h>
#include <cui/cui_render.h>
#include "../examples/gallery_ui.h"

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

typedef void (*setup_fn)(gallery *g);

static float g_scale = 2;
static int shoot(const char *path, int page, bool dark, int accent, float W, float H, int frames,
                 void (*interact)(cui_ctx *ui, int frame, gallery *g)) {
    cui_sw_renderer *sw = cui_sw_create();
    cui_ctx *ui = cui_create(&(cui_ctx_desc){ .renderer = cui_sw_iface(sw), .theme = dark ? CUI_THEME_DARK : CUI_THEME_LIGHT,
                                              .accent = (cui_accent)accent,
                                              .direction = getenv("CUI_GALLERY_RTL") ? CUI_DIRECTION_RTL : CUI_DIRECTION_INHERIT });
    cui_set_viewport(ui, W, H, g_scale);
    gallery g;
    gallery_init(&g);
    g.page = page;
    g.dark = dark;
    g.accent = accent;
    cui_input_mouse_leave(ui);
    for (int f = 0; f < frames; f++) {
        if (interact) interact(ui, f, &g);
        cui_begin_frame(ui, 1.0f / 60.0f);
        gallery_frame(ui, &g);
        cui_end_frame(ui);
    }
    const cui_draw_data *dd = cui_get_draw_data(ui);
    int w = dd->width_px, h = dd->height_px;
    unsigned char *px = malloc((size_t)w * h * 4);
    cui_sw_render(sw, dd, px, w, h, w * 4, CUI_SW_RGBA);
    bool ok = cui_write_png(path, px, w, h, w * 4);
    printf("%s %s (%d vertices)\n", ok ? "wrote" : "FAILED", path, dd->vertex_count);
    free(px);
    cui_destroy(ui);
    cui_sw_destroy(sw);
    return ok ? 0 : 1;
}

static void open_dialog(cui_ctx *ui, int f, gallery *g) { (void)ui; if (f == 5) g->dialog_open = true; }
static void open_palette(cui_ctx *ui, int f, gallery *g) { (void)ui; if (f == 5) g->palette_open = true; }
static void open_sheet(cui_ctx *ui, int f, gallery *g) { (void)ui; if (f == 5) g->sheet_open = true; }
static void toasts(cui_ctx *ui, int f, gallery *g) {
    (void)g;
    if (f == 5) cui_toast(ui, "Upload failed", .description = "The file is larger than 25 MB.", .tone = CUI_TONE_DANGER);
    if (f == 10) cui_toast(ui, "Message archived", .action = "Undo");
    if (f == 15) cui_toast(ui, "Changes saved", .description = "Your profile is up to date.", .tone = CUI_TONE_SUCCESS);
}
static void hover_chart(cui_ctx *ui, int f, gallery *g) {
    (void)g;
    if (f == 30) cui_input_mouse_move(ui, 760, 300);
}

int main(int argc, char **argv) {
    const char *dir = argc > 1 ? argv[1] : ".";
    int only = argc > 2 ? atoi(argv[2]) : -1;
    if (argc > 3) g_scale = (float)atof(argv[3]);
    int fails = 0;
    char path[512];
    static const char *slugs[PAGE_COUNT] = { "overview", "buttons", "inputs", "selection", "navigation", "overlays", "feedback", "data", "charts" };
    for (int p = 0; p < PAGE_COUNT; p++) {
        if (only >= 0 && only != p) continue;
        for (int d = 0; d < 2; d++) {
            /* "page-" keeps these apart from the component shots of the same
             * name (buttons, inputs) when both land in one folder */
            snprintf(path, sizeof path, "%s/page-%s-%s.png", dir, slugs[p], d ? "dark" : "light");
            fails += shoot(path, p, d, 0, 1240, p == PAGE_DATA ? 1500 : 1000, 90, p == PAGE_CHARTS ? hover_chart : NULL);
        }
    }
    for (int p = PAGE_COUNT; p < gallery_page_total(); p++) {
        if (only >= 0 && only != p) continue;
        char slug[64];
        int n = 0;
        for (const char *c = gallery_page_name(p); *c && n < 60; c++) slug[n++] = *c == ' ' ? '-' : (char)((*c >= 'A' && *c <= 'Z') ? *c + 32 : *c);
        slug[n] = 0;
        for (int d = 0; d < 2; d++) {
            snprintf(path, sizeof path, "%s/more-%s-%s.png", dir, slug, d ? "dark" : "light");
            fails += shoot(path, p, d, 0, 1240, 1500, 90, NULL);
        }
    }
    if (only < 0) {
        snprintf(path, sizeof path, "%s/dialog-light.png", dir);
        fails += shoot(path, PAGE_OVERLAYS, false, 0, 1240, 800, 90, open_dialog);
        snprintf(path, sizeof path, "%s/palette-dark.png", dir);
        fails += shoot(path, PAGE_OVERVIEW, true, 2, 1240, 800, 90, open_palette);
        snprintf(path, sizeof path, "%s/sheet-light.png", dir);
        fails += shoot(path, PAGE_OVERLAYS, false, 1, 1240, 800, 90, open_sheet);
        snprintf(path, sizeof path, "%s/toasts-light.png", dir);
        fails += shoot(path, PAGE_FEEDBACK, false, 0, 1240, 800, 60, toasts);
        snprintf(path, sizeof path, "%s/overview-light.png", dir);
        fails += shoot(path, PAGE_OVERVIEW, false, 0, 1240, 1000, 90, NULL);
        snprintf(path, sizeof path, "%s/overview-green-dark.png", dir);
        fails += shoot(path, PAGE_OVERVIEW, true, CUI_ACCENT_GREEN, 1240, 1000, 90, NULL);
        snprintf(path, sizeof path, "%s/selection-blue-light.png", dir);
        fails += shoot(path, PAGE_SELECTION, false, 2, 1240, 1000, 90, NULL);
    }
    return fails ? 1 : 0;
}
