#if !defined(_WIN32) && !defined(_POSIX_C_SOURCE)
#define _POSIX_C_SOURCE 200809L
#endif
/* Measures wall time per frame (build + layout + paint) for gallery pages,
 * and software rasterization time. */
#include <cui/cui.h>
#include <cui/cui_render.h>
#include "../examples/gallery_ui.h"
#include <stdio.h>
#include <stdlib.h>
#include <time.h>
#if defined(_WIN32)
#include <windows.h>
static double now(void) { LARGE_INTEGER f, c; QueryPerformanceFrequency(&f); QueryPerformanceCounter(&c); return (double)c.QuadPart / f.QuadPart; }
#else
static double now(void) { struct timespec t; clock_gettime(CLOCK_MONOTONIC, &t); return (double)t.tv_sec + (double)t.tv_nsec * 1e-9; }
#endif

int main(void) {
    cui_sw_renderer *sw = cui_sw_create();
    cui_ctx *ui = cui_create(&(cui_ctx_desc){ .renderer = cui_sw_iface(sw) });
    cui_set_viewport(ui, 1240, 820, 2);
    gallery g;
    gallery_init(&g);
    printf("%-14s %10s %10s %10s\n", "page", "frame ms", "vertices", "raster ms");
    unsigned char *px = malloc(2480 * 1640 * 4);
    for (int p = 0; p < gallery_page_total(); p++) {
        g.page = p;
        for (int i = 0; i < 30; i++) { cui_begin_frame(ui, 1 / 60.0f); gallery_frame(ui, &g); cui_end_frame(ui); }
        int n = 200;
        double t0 = now();
        for (int i = 0; i < n; i++) {
            cui_input_mouse_move(ui, 300 + (i % 50) * 10.0f, 400);
            cui_begin_frame(ui, 1 / 60.0f); gallery_frame(ui, &g); cui_end_frame(ui);
        }
        double frame = (now() - t0) / n * 1000;
        const cui_draw_data *dd = cui_get_draw_data(ui);
        double r0 = now();
        for (int i = 0; i < 5; i++) cui_sw_render(sw, dd, px, dd->width_px, dd->height_px, dd->width_px * 4, CUI_SW_BGRA);
        double raster = (now() - r0) / 5 * 1000;
        printf("%-14s %10.3f %10d %10.2f\n", gallery_page_name(p), frame, dd->vertex_count, raster);
    }
    free(px);
    cui_destroy(ui);
    cui_sw_destroy(sw);
    return 0;
}
