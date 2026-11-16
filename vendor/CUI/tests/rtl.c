/* Headless RTL display probe. Pass a trusted Vazirmatn-Regular.ttf and PNG path. */
#include <cui/cui.h>
#include <cui/cui_render.h>
#include <math.h>
#include <stdio.h>
#include <stdlib.h>

static const char *samples[] = {
    "سلام دنیا",
    "امروز ۳ تا Timer دارم",
    "می‌خوام",
    "میخوام",
    "لا",
    "Timer: سلام ۱۲۳ دنیا.",
    "(سلام) Timer 123"
};
static const char *paragraph = "امروز می‌خوام یه متن طولانی فارسی بنویسم تا ببینم وقتی خط‌ها شکسته می‌شن، واژه‌ها درست دیده می‌شن یا نه. سه تا Timer دارم و باید همه‌شون رو تا فردا بررسی کنم. سلام دنیا، این یه آزمایش برای نمایش نوشته‌های فارسی در چند خطه.";

int main(int argc, char **argv) {
    if (argc < 3) { fprintf(stderr, "usage: cui_rtl FONT.ttf OUT.png\n"); return 2; }
    cui_sw_renderer *sw = cui_sw_create();
    cui_ctx *ui = cui_create(&(cui_ctx_desc){ .renderer = cui_sw_iface(sw), .theme = argc > 3 ? CUI_THEME_DARK : CUI_THEME_LIGHT });
    cui_font font = cui_font_load_file(ui, argv[1]);
    if (font < 0) return 1;
    cui_font_add_fallback(ui, CUI_FONT_BODY, font);
#ifdef CUI_TEXT_SHAPING
    float fallback = cui_measure_text(ui, CUI_FONT_BODY, 32, "سلام", -1).width;
    float direct = cui_measure_text(ui, font, 32, "سلام", -1).width;
    if (fabsf(fallback - direct) > 0.01f) { fprintf(stderr, "fallback width mismatch\n"); return 1; }
    float la = cui_measure_text(ui, font, 32, "لا", -1).width;
    float separate = cui_measure_text(ui, font, 32, "ل", -1).width + cui_measure_text(ui, font, 32, "ا", -1).width;
    float zwnj = cui_measure_text(ui, font, 32, samples[2], -1).width;
    float joined = cui_measure_text(ui, font, 32, samples[3], -1).width;
    if (la >= separate * 0.85f || fabsf(zwnj - joined) < 0.5f) {
        fprintf(stderr, "lam-alef or ZWNJ shaping failed: %g %g %g %g\n", la, separate, zwnj, joined);
        return 1;
    }
#endif
    cui_font_add_fallback(ui, CUI_FONT_MEDIUM, font);
    cui_set_viewport(ui, 560, 360, 2);
    const char *labels[] = { "3) Half-space (ZWNJ):", "3) Half-space (ZWNJ): می‌خوام" };
    int vertices[2];
    for (int i = 0; i < 2; i++) {
        cui_begin_frame(ui, 1.0f / 60);
        cui_label(ui, labels[i]);
        cui_end_frame(ui);
        vertices[i] = cui_get_draw_data(ui)->vertex_count;
    }
    if (vertices[1] <= vertices[0]) {
        fprintf(stderr, "mixed label lost its Persian fallback glyphs\n");
        return 1;
    }
    cui_font_set_role(ui, CUI_FONT_BODY, font);
    cui_set_viewport(ui, 900, 1000, 1);
    for (int f = 0; f < 3; f++) {
        cui_begin_frame(ui, 1.0f / 60);
        cui_column(ui, .pad_x = 24, .pad_top = 704, .width = CUI_PX(768)) {
            cui_text(ui, paragraph, .size = 24);
        }
        cui_end_frame(ui);
    }
    /* Direct drawing after layout keeps each baseline identical to the HTML fixture. */
    cui_begin_frame(ui, 1.0f / 60);
    cui_draw_rect(ui, (cui_rect){0, 0, 900, 1000}, CUI_CORNERS(0), CUI_TOKEN_COLOR(CUI_TOKEN_BACKGROUND));
    for (int i = 0; i < 7; i++) {
        cui_draw_text(ui, font, 32, (cui_vec2){24, 42.0f + (float)i * 84}, CUI_TOKEN_COLOR(CUI_TOKEN_FOREGROUND), samples[i], -1);
        printf("line %d width %.6f\n", i + 1, cui_measure_text(ui, font, 32, samples[i], -1).width);
    }
    cui_column(ui, .pad_x = 24, .pad_top = 704, .width = CUI_PX(768)) {
        cui_text(ui, paragraph, .size = 24);
    }
    cui_end_frame(ui);
    const cui_draw_data *dd = cui_get_draw_data(ui);
    unsigned char *pixels = malloc((size_t)dd->width_px * dd->height_px * 4);
    if (!pixels) return 1;
    cui_sw_render(sw, dd, pixels, dd->width_px, dd->height_px, dd->width_px * 4, CUI_SW_RGBA);
    int ok = cui_write_png(argv[2], pixels, dd->width_px, dd->height_px, dd->width_px * 4);
    free(pixels);
    cui_destroy(ui);
    cui_sw_destroy(sw);
#ifdef CUI_TEXT_SHAPING
    printf("lam-alef, ZWNJ, fallback checks passed; wrote %s\n", argv[2]);
#else
    printf("shaping OFF; wrote %s\n", argv[2]);
#endif
    return ok ? 0 : 1;
}
