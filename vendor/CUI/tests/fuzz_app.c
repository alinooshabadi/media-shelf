/* Drives one example app with random input, like fuzz_gallery.c. The example
 * source is included with its main() renamed; FUZZ_APP names the file.
 *   cui_fuzz_<app> [frames] [seed] */
#define main example_main
#include FUZZ_APP
#undef main

#include <cui/cui_render.h>
#include <stdlib.h>

static unsigned rng_state;
static unsigned rnd(void) { rng_state = rng_state * 1664525u + 1013904223u; return rng_state >> 8; }
static float frand(float lo, float hi) { return lo + (hi - lo) * (float)(rnd() % 10000) / 10000.0f; }

static char clip[256] = "pasted \xc3\xa9";
static void set_clip(void *u, const char *s) { (void)u; snprintf(clip, sizeof clip, "%s", s); }
static const char *get_clip(void *u) { (void)u; return clip; }

int main(int argc, char **argv) {
    int frames = argc > 1 ? atoi(argv[1]) : 5000;
    rng_state = argc > 2 ? (unsigned)atoi(argv[2]) : 777u;
    cui_sw_renderer *sw = cui_sw_create();
    cui_ctx *ui = cui_create(&(cui_ctx_desc){ .renderer = cui_sw_iface(sw), .theme = CUI_THEME_DARK });
    cui_set_platform(ui, &(cui_platform_iface){ .set_clipboard = set_clip, .get_clipboard = get_clip });
    static app_state state;
    float W = 1440, H = 900;
    cui_set_viewport(ui, W, H, 1);
    static const char *texts[] = { "a", "Hello", " ", "\xc3\xa9", "\xf0\x9f\x98\x80", "42" };
    static const cui_key keys[] = { CUI_KEY_TAB, CUI_KEY_ENTER, CUI_KEY_ESCAPE, CUI_KEY_BACKSPACE, CUI_KEY_SPACE,
                                    CUI_KEY_LEFT, CUI_KEY_RIGHT, CUI_KEY_UP, CUI_KEY_DOWN, CUI_KEY_K, CUI_KEY_V };
    for (int f = 0; f < frames; f++) {
        int n = (int)(rnd() % 4);
        for (int e = 0; e < n; e++) {
            switch (rnd() % 9) {
            case 0: case 1: case 2: cui_input_mouse_move(ui, frand(-20, W + 20), frand(-20, H + 20)); break;
            case 3: cui_input_mouse_button(ui, (cui_mouse_button)(rnd() % 3), rnd() % 2); break;
            case 4: cui_input_scroll(ui, frand(-3, 3), frand(-5, 5)); break;
            case 5: case 6: cui_input_key(ui, keys[rnd() % (sizeof keys / sizeof keys[0])], true, (int)(rnd() % 16)); break;
            case 7: cui_input_text(ui, texts[rnd() % (sizeof texts / sizeof texts[0])]); break;
            default: if (rnd() % 60 == 0) { W = frand(360, 1800); H = frand(280, 1100); cui_set_viewport(ui, W, H, (rnd() % 3) ? 1.0f : 2.0f); } break;
            }
        }
        if (rnd() % 400 == 0) cui_set_theme(ui, (cui_theme_mode)(rnd() % 2));
        cui_begin_frame(ui, frand(0.0f, 0.05f));
        frame(ui, &state);
        cui_end_frame(ui);
        if (f % 500 == 0) {
            const cui_draw_data *dd = cui_get_draw_data(ui);
            unsigned char *px = malloc((size_t)dd->width_px * dd->height_px * 4);
            cui_sw_render(sw, dd, px, dd->width_px, dd->height_px, dd->width_px * 4, CUI_SW_RGBA);
            free(px);
        }
    }
    printf("fuzzed %d frames OK\n", frames);
    cui_destroy(ui);
    cui_sw_destroy(sw);
    return 0;
}
