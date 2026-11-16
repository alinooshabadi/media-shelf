/* Soak test: runs an app the way a person would, for a long time, and records
 * memory and frame times. The app's source is included with cui_app_run
 * intercepted, so any example (or any app written the same way) runs with its
 * own data.
 *
 *   cui_soak_<app> [options]
 *     --minutes M     how long (simulated minutes headless, real minutes windowed); default 10
 *     --seed N        input seed
 *     --window        open a real window (X11) and drive it with real input events (XTest)
 *     --sw            windowed: software renderer instead of OpenGL
 *     --idle          windowed, with no input at all: the cost of an app left alone
 *     --storm         headless: a new window size and display scale every 10 frames
 *     --csv PATH      a sample every few seconds: time, memory, frame times, counts
 *     --sites         where live memory and later allocations come from (build with -fno-omit-frame-pointer)
 *
 * Headless, the context gets a counting allocator, frames advance 1/60 s each
 * and run as fast as they can, so an hour of use takes a minute or two.
 * The person: moves the pointer to a control the app drew last frame and
 * clicks it, types into whatever field has focus, scrolls what scrolls, drags,
 * presses Escape, Tab and arrows, sometimes resizes the window or switches
 * theme, and sometimes leaves it alone for a while. */
#if defined(__linux__) && !defined(_GNU_SOURCE)
#define _GNU_SOURCE 1   /* dladdr, for --sites */
#endif
#define cui_app_run soak_app_run
#define cui_app_file_dialog soak_file_dialog
#define main example_main
#include SOAK_APP
#undef main
#undef cui_app_run
#undef cui_app_file_dialog

#include "../src/cui_internal.h"
#include "../src/platform/cui_platform.h"
#include <cui/cui_render.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>
#if defined(__linux__)
#include <dlfcn.h>
#include <malloc.h>
#include <sys/resource.h>
#include <unistd.h>
#endif

/* ---- options ---- */
static double opt_minutes = 10;
static unsigned opt_seed = 1;
static bool opt_window, opt_sw, opt_full, opt_idle, opt_storm;
/* --hash: a fingerprint of every frame's draw data. Runs are deterministic (seeded input, fixed
 * time step), so a refactor that keeps behavior keeps the hash; any change in what is drawn,
 * anywhere in the run, changes it. */
static bool opt_hash;
static uint64_t frame_hash = 1469598103934665603ull;
static void hash_bytes(const void *p, size_t n) {
    const unsigned char *b = p;
    for (size_t i = 0; i < n; i++) { frame_hash ^= b[i]; frame_hash *= 1099511628211ull; }
}
static void hash_draw_data(const cui_draw_data *dd) {
    hash_bytes(dd->vertices, sizeof dd->vertices[0] * (size_t)dd->vertex_count);
    hash_bytes(dd->indices, sizeof dd->indices[0] * (size_t)dd->index_count);
    for (int i = 0; i < dd->command_count; i++) {
        const cui_draw_cmd *c = &dd->commands[i];
        hash_bytes(&c->clip, sizeof c->clip); hash_bytes(&c->index_offset, sizeof c->index_offset); hash_bytes(&c->index_count, sizeof c->index_count);
    }
}
static const char *opt_csv;

bool soak_file_dialog(cui_app *app, const cui_file_dialog_opts *o, char *out, size_t cap) {
    (void)app; (void)o;
    snprintf(out, cap, "/tmp/soak-file.txt");
    return true;
}

/* ---- random ---- */
static unsigned rng;
static unsigned rnd(void) { rng = rng * 1664525u + 1013904223u; return rng >> 8; }
static float frand(float lo, float hi) { return lo + (hi - lo) * (float)(rnd() % 100000) / 100000.0f; }
static bool chance(float p) { return frand(0, 1) < p; }

/* ---- clocks and process numbers ---- */
static double now_s(void) {
    struct timespec t;
    clock_gettime(CLOCK_MONOTONIC, &t);
    return (double)t.tv_sec + t.tv_nsec * 1e-9;
}
static double rss_mb(void) {
#if defined(__linux__)
    long pages = 0, res = 0;
    FILE *f = fopen("/proc/self/statm", "r");
    if (f) { if (fscanf(f, "%ld %ld", &pages, &res) != 2) res = 0; fclose(f); }
    return (double)res * (double)sysconf(_SC_PAGESIZE) / (1024.0 * 1024.0);
#else
    return 0;
#endif
}
static double heap_mb(void) {
#if defined(__linux__) && defined(__GLIBC__) && (__GLIBC__ > 2 || (__GLIBC__ == 2 && __GLIBC_MINOR__ >= 33))
    struct mallinfo2 m = mallinfo2();
    return (double)m.uordblks / (1024.0 * 1024.0);
#else
    return 0;
#endif
}
static double cpu_s(void) {
#if defined(__linux__)
    struct rusage u;
    getrusage(RUSAGE_SELF, &u);
    return u.ru_utime.tv_sec + u.ru_utime.tv_usec * 1e-6 + u.ru_stime.tv_sec + u.ru_stime.tv_usec * 1e-6;
#else
    return 0;
#endif
}

/* ---- counting allocator (headless) ---- */
typedef struct { size_t size; void *site; } soak_hdr;   /* site: who asked, for --sites */
static size_t live_bytes, peak_bytes;
static unsigned long long alloc_calls;
static bool opt_sites, opt_deep;   /* --deep: name the caller one level further up */
#define SOAK_LIVE (1 << 16)
static soak_hdr *live_tab[SOAK_LIVE];
static void live_add(soak_hdr *h) { if (!opt_sites) return; size_t i = ((uintptr_t)h >> 4) & (SOAK_LIVE - 1); while (live_tab[i]) i = (i + 1) & (SOAK_LIVE - 1); live_tab[i] = h; }
static void live_del(soak_hdr *h) {
    if (!opt_sites) return;
    size_t i = ((uintptr_t)h >> 4) & (SOAK_LIVE - 1);
    while (live_tab[i] && live_tab[i] != h) i = (i + 1) & (SOAK_LIVE - 1);
    if (!live_tab[i]) return;
    live_tab[i] = NULL;
    /* re-place the rest of the run */
    for (size_t j = (i + 1) & (SOAK_LIVE - 1); live_tab[j]; j = (j + 1) & (SOAK_LIVE - 1)) { soak_hdr *x = live_tab[j]; live_tab[j] = NULL; live_add(x); }
}
static void dump_sites(void) {
    typedef struct { void *site; size_t bytes; int n; } agg;
    static agg a[4096];
    int na = 0;
    for (size_t i = 0; i < SOAK_LIVE; i++) {
        soak_hdr *h = live_tab[i];
        if (!h) continue;
        int k = 0;
        while (k < na && a[k].site != h->site) k++;
        if (k == na && na < 4096) { a[na].site = h->site; a[na].bytes = 0; a[na].n = 0; na++; }
        if (k < na) { a[k].bytes += h->size; a[k].n++; }
    }
    for (int i = 0; i < na; i++) for (int j = i + 1; j < na; j++) if (a[j].bytes > a[i].bytes) { agg t = a[i]; a[i] = a[j]; a[j] = t; }
    printf("  live memory by caller (resolve with addr2line -f -e <binary>):\n");
    for (int i = 0; i < na && i < 14; i++) {
        Dl_info di;
        uintptr_t off = (uintptr_t)a[i].site;
        if (dladdr(a[i].site, &di) && di.dli_fbase) off -= (uintptr_t)di.dli_fbase;
        printf("    %8zu bytes in %4d blocks  0x%lx\n", a[i].bytes, a[i].n, (unsigned long)off);
    }
}
static bool warm;   /* after the first minute: count calls by caller */
typedef struct { void *site, *site2; unsigned long calls; size_t bytes; } soak_calls;
static soak_calls calls_tab[512];
static void count_call(void *site, void *site2, size_t n) {
    if (!warm || !opt_sites) return;
    for (int i = 0; i < 512; i++) {
        if ((calls_tab[i].site == site && calls_tab[i].site2 == site2) || !calls_tab[i].site) { calls_tab[i].site = site; calls_tab[i].site2 = site2; calls_tab[i].calls++; calls_tab[i].bytes += n; return; }
    }
}
static void dump_calls(void) {
    for (int i = 0; i < 512 && calls_tab[i].site; i++) for (int j = i + 1; j < 512 && calls_tab[j].site; j++)
        if (calls_tab[j].calls > calls_tab[i].calls) { soak_calls t = calls_tab[i]; calls_tab[i] = calls_tab[j]; calls_tab[j] = t; }
    printf("  allocation calls after warm-up by caller:\n");
    for (int i = 0; i < 12 && calls_tab[i].site; i++) {
        Dl_info di;
        uintptr_t off = (uintptr_t)calls_tab[i].site, off2 = (uintptr_t)calls_tab[i].site2;
        if (dladdr(calls_tab[i].site, &di) && di.dli_fbase) { off -= (uintptr_t)di.dli_fbase; off2 -= (uintptr_t)di.dli_fbase; }
        printf("    %8lu calls, %10zu bytes  0x%lx 0x%lx\n", calls_tab[i].calls, calls_tab[i].bytes, (unsigned long)off, (unsigned long)off2);
    }
}
static void *soak_alloc(size_t n, void *u) {
    (void)u;
    soak_hdr *h = malloc(sizeof *h + n);
    if (!h) return NULL;
    live_add(h);
    h->size = n; h->site = opt_sites ? __builtin_return_address(1) : NULL; live_bytes += n; alloc_calls++;
    if (opt_sites) count_call(h->site, (opt_deep ? __builtin_return_address(3) : __builtin_return_address(2)), n);
    if (live_bytes > peak_bytes) peak_bytes = live_bytes;
    return h + 1;
}
static void soak_free(void *p, void *u) {
    (void)u;
    if (!p) return;
    soak_hdr *h = (soak_hdr *)p - 1;
    live_bytes -= h->size;
    live_del(h);
    free(h);
}
static void *soak_realloc(void *p, size_t n, void *u) {
    if (!p) return soak_alloc(n, u);
    soak_hdr *h = (soak_hdr *)p - 1;
    size_t old = h->size;
    live_del(h);
    soak_hdr *g = realloc(h, sizeof *g + n);
    if (!g) { live_add(h); return NULL; }
    live_add(g);
    live_bytes = live_bytes - old + n; alloc_calls++;
    g->size = n; g->site = opt_sites ? __builtin_return_address(1) : NULL;
    if (opt_sites) count_call(g->site, (opt_deep ? __builtin_return_address(3) : __builtin_return_address(2)), n);
    if (live_bytes > peak_bytes) peak_bytes = live_bytes;
    return g + 1;
}

/* ---- textures, counted through the renderer ---- */
static cui_renderer_iface inner;
static long tex_live, tex_bytes;
typedef struct { cui_texture t; long bytes; } soak_tex;
static soak_tex texs[4096];
static cui_texture soak_tex_create(void *u, int w, int h, cui_texture_format f, const void *px) {
    cui_texture t = inner.texture_create(u, w, h, f, px);
    if (t) {
        tex_live++; tex_bytes += (long)w * h * (long)f;
        for (int i = 0; i < 4096; i++) if (!texs[i].t) { texs[i].t = t; texs[i].bytes = (long)w * h * (long)f; break; }
    }
    return t;
}
static void soak_tex_destroy(void *u, cui_texture t) {
    for (int i = 0; i < 4096; i++) if (texs[i].t == t) { tex_bytes -= texs[i].bytes; texs[i].t = 0; tex_live--; break; }
    inner.texture_destroy(u, t);
}

/* ---- frame time statistics ---- */
typedef struct { float v[1 << 16]; int n; double sum; float max; } soak_stat;
static void stat_add(soak_stat *s, float x) {
    if (s->n < (1 << 16)) s->v[s->n++] = x;
    else s->v[rnd() % (1 << 16)] = x;   /* a reservoir once full */
    s->sum += x;
    if (x > s->max) s->max = x;
}
static int cmpf(const void *a, const void *b) { float x = *(const float *)a, y = *(const float *)b; return x < y ? -1 : x > y; }
static float stat_pct(soak_stat *s, float p) {
    if (!s->n) return 0;
    qsort(s->v, (size_t)s->n, sizeof(float), cmpf);
    return s->v[CUI_MIN(s->n - 1, (int)(p * s->n))];
}

/* ---- the slowest frames, with what happened just before ---- */
typedef struct { float ms; double t; char what[80]; } soak_slow;
static soak_slow slowest[5], slowest_paint[5];
static char last_action[48] = "start";
static void note_slow_in(soak_slow *list, float ms, double t, const char *extra) {
    int k = 0;
    for (int i = 1; i < 5; i++) if (list[i].ms < list[k].ms) k = i;
    if (ms > list[k].ms) { list[k].ms = ms; list[k].t = t; snprintf(list[k].what, sizeof list[k].what, "%s%s", last_action, extra); }
}
static void note_slow(float ms, double t) { note_slow_in(slowest, ms, t, ""); }

/* ---- the person ---- */
typedef enum { A_NONE, A_MOVE, A_DOWN, A_UP, A_SCROLL, A_KEY, A_TEXT, A_RESIZE, A_THEME, A_WAIT } soak_kind;
typedef struct { soak_kind k; float x, y; int key, mods; char text[48]; double at; } soak_act;
static soak_act queue[256];
static int qhead, qtail;
static void push(soak_act a) { if ((qtail + 1) % 256 != qhead) { queue[qtail] = a; qtail = (qtail + 1) % 256; } }
static bool empty(void) { return qhead == qtail; }

typedef struct { cui_rect r; uint32_t flags; } soak_target;
static soak_target targets[2048];
static int ntargets;
static bool wants_kb;
static float px_, py_, vw_ = 1280, vh_ = 800;
static long n_clicks, n_types, n_scrolls, n_drags, n_keys, n_resizes, n_idles;

static const char *const words[] = { "hello", "release", "notes", "draft", "budget", "design", "review", "ship it", "42", "Q3",
                                     "caf\xc3\xa9", "na\xc3\xafve", "\xf0\x9f\x8e\x89", "\xd9\x85\xd8\xb1\xd8\xad\xd8\xa8\xd8\xa7", "\xd7\xa9\xd7\x9c\xd7\x95\xd7\x9d" };

/* plans the next few steps, starting at time t */
static void plan(double t, bool ascii_only) {
    soak_act a;
    memset(&a, 0, sizeof a);
    float r = frand(0, 1);
    if (r < 0.04f) {   /* leave it alone a while: an idle app should cost nothing */
        a.k = A_WAIT; a.at = t + frand(2, 12); push(a); n_idles++;
        return;
    }
    if (wants_kb && r < 0.30f) {   /* typing into the field that has focus */
        int nw = 1 + (int)(rnd() % 4);
        double at = t;
        for (int i = 0; i < nw; i++) {
            const char *w = words[rnd() % (ascii_only ? 10 : sizeof words / sizeof *words)];
            a.k = A_TEXT; snprintf(a.text, sizeof a.text, "%s%s", i ? " " : "", w); at += frand(0.15f, 0.6f); a.at = at; push(a);
        }
        if (chance(0.3f)) { a.k = A_KEY; a.key = CUI_KEY_BACKSPACE; a.mods = 0; at += 0.2; a.at = at; push(a); }
        if (chance(0.2f)) { a.k = A_KEY; a.key = CUI_KEY_A; a.mods = CUI_MOD_PRIMARY; at += 0.3; a.at = at; push(a); }
        if (chance(0.4f)) { a.k = A_KEY; a.key = CUI_KEY_ENTER; a.mods = 0; at += 0.3; a.at = at; push(a); }
        n_types++;
        return;
    }
    if (r < 0.40f || ntargets == 0) {   /* keys: Escape, Tab, arrows */
        static const int keys[] = { CUI_KEY_ESCAPE, CUI_KEY_TAB, CUI_KEY_TAB, CUI_KEY_DOWN, CUI_KEY_UP, CUI_KEY_LEFT, CUI_KEY_RIGHT, CUI_KEY_ENTER, CUI_KEY_SPACE, CUI_KEY_PAGE_DOWN, CUI_KEY_HOME };
        a.k = A_KEY; a.key = keys[rnd() % (sizeof keys / sizeof *keys)]; a.mods = chance(0.15f) ? CUI_MOD_SHIFT : 0; a.at = t + frand(0.1f, 0.5f); push(a);
        n_keys++;
        return;
    }
    soak_target *tg = &targets[rnd() % ntargets];
    float tx = tg->r.x + tg->r.w * frand(0.25f, 0.75f), ty = tg->r.y + tg->r.h * frand(0.25f, 0.75f);
    tx = CUI_CLAMP(tx, 1, vw_ - 2); ty = CUI_CLAMP(ty, 1, vh_ - 2);
    /* glide there over a few frames */
    double at = t;
    int steps = 4 + (int)(rnd() % 8);
    for (int i = 1; i <= steps; i++) {
        a.k = A_MOVE; a.x = px_ + (tx - px_) * i / steps; a.y = py_ + (ty - py_) * i / steps; at += 1.0 / 60; a.at = at; push(a);
    }
    px_ = tx; py_ = ty;
    at += frand(0.05f, 0.3f);
    bool scrollable = (tg->flags & (CUI_NF_SCROLL_X | CUI_NF_SCROLL_Y)) != 0;
    if (scrollable && chance(0.6f)) {
        int n = 1 + (int)(rnd() % 6);
        float dir = chance(0.6f) ? -1.0f : 1.0f;
        for (int i = 0; i < n; i++) { a.k = A_SCROLL; a.x = 0; a.y = dir * frand(0.5f, 3); at += 0.05; a.at = at; push(a); }
        n_scrolls++;
    } else if (chance(0.12f)) {   /* a drag: sliders, splitters, cards, canvases */
        a.k = A_DOWN; a.at = at; push(a);
        float dx = frand(-160, 160), dy = frand(-90, 90);
        for (int i = 1; i <= 12; i++) { a.k = A_MOVE; a.x = CUI_CLAMP(tx + dx * i / 12, 1, vw_ - 2); a.y = CUI_CLAMP(ty + dy * i / 12, 1, vh_ - 2); at += 1.0 / 60; a.at = at; push(a); }
        a.k = A_UP; at += 0.05; a.at = at; push(a);
        px_ = CUI_CLAMP(tx + dx, 1, vw_ - 2); py_ = CUI_CLAMP(ty + dy, 1, vh_ - 2);
        n_drags++;
    } else {
        a.k = A_DOWN; a.at = at; push(a);
        a.k = A_UP; at += frand(0.06f, 0.14f); a.at = at; push(a);
        if (chance(0.06f)) { a.k = A_DOWN; at += 0.08; a.at = at; push(a); a.k = A_UP; at += 0.08; a.at = at; push(a); }   /* double click */
        n_clicks++;
    }
    if (chance(0.006f)) {   /* a new window size: phone, tablet, desktop */
        static const float sizes[][2] = { { 390, 844 }, { 820, 1180 }, { 1280, 800 }, { 1440, 900 }, { 1024, 700 }, { 600, 500 } };
        int k = (int)(rnd() % 6);
        a.k = A_RESIZE; a.x = sizes[k][0]; a.y = sizes[k][1]; at += 0.5; a.at = at; push(a);
        n_resizes++;
    }
    if (chance(0.003f)) { a.k = A_THEME; at += 0.2; a.at = at; push(a); }
}

/* the controls of the last frame, as targets */
static void collect_targets(cui_ctx *ui) {
    ntargets = 0;
    for (int i = 0; i < ui->prev_hit_count && ntargets < 2048; i++) {
        cui_hit *h = &ui->prev_hits[i];
        if (h->rect.w < 3 || h->rect.h < 3) continue;
        if (h->rect.x > vw_ || h->rect.y > vh_ || h->rect.x + h->rect.w < 0 || h->rect.y + h->rect.h < 0) continue;
        targets[ntargets].r = h->rect;
        targets[ntargets].flags = h->flags;
        ntargets++;
    }
    wants_kb = cui_wants_keyboard(ui);
}

/* ---- a sample line ---- */
static FILE *csv;
static double t0_cpu, t0_wall;
static void sample(cui_ctx *ui, double t, soak_stat *build, soak_stat *paint, long frames, long drawn) {
    double cpu = cpu_s() - t0_cpu, wall = now_s() - t0_wall;
    if (csv) {
        fprintf(csv, "%.1f,%.3f,%.3f,%.3f,%llu,%ld,%ld,%d,%d,%d,%ld,%ld,%.3f,%.3f,%.1f,%.1f\n", t, live_bytes / 1048576.0, heap_mb(), rss_mb(),
                alloc_calls, tex_live, tex_bytes / 1024, ui->state_count, ui->node_count, ui->dl.vtx_count, frames, drawn,
                build->n ? build->sum / build->n : 0.0, paint->n ? paint->sum / paint->n : 0.0, cpu, wall);
        fflush(csv);
    }
}

/* ---- headless ---- */
static int run_headless(const cui_app_desc *desc, void (*frame)(cui_ctx *, void *), void *user) {
    cui_sw_renderer *sw = cui_sw_create();
    cui_sw_set_threads(sw, 1);
    cui_sw_set_incremental(sw, true);   /* as the app's software path does */
    inner = cui_sw_iface(sw);
    cui_renderer_iface wrap = inner;
    wrap.texture_create = soak_tex_create; wrap.texture_destroy = soak_tex_destroy;
    cui_ctx *ui = cui_create(&(cui_ctx_desc){ .renderer = wrap, .allocator = { soak_alloc, soak_realloc, soak_free, NULL },
                                               .theme = desc ? desc->theme : CUI_THEME_LIGHT, .accent = desc ? desc->accent : CUI_ACCENT_NEUTRAL });
    vw_ = desc && desc->width ? (float)desc->width : 1080; vh_ = desc && desc->height ? (float)desc->height : 720;
    cui_set_viewport(ui, vw_, vh_, 2);
    cui_input_focus(ui, true);
    static soak_stat build, paint, build_steady;
    unsigned char *px = NULL, *px_check = NULL;
    size_t px_cap = 0;
    double painted_px = 0, total_px = 0;
    long checks = 0, mismatches = 0;
    double t = 0, end = opt_minutes * 60, next_sample = 0;
    long frames = 0, drawn = 0, idle_frames = 0;
    size_t live_at_warm = 0, min_live_after = (size_t)-1, max_live_after = 0;
    unsigned long long allocs_at_warm = 0;
    long frames_at_warm = 0;
    double plan_at = 1.0, wake_at = -1;
    bool waiting = false;
    t0_cpu = cpu_s(); t0_wall = now_s();
    while (t < end) {
        /* input due by now */
        if (empty() && t >= plan_at) { waiting = false; plan(t, false); }
        while (!empty() && queue[qhead].at <= t) {
            soak_act a = queue[qhead];
            qhead = (qhead + 1) % 256;
            if (a.k != A_MOVE && a.k != A_WAIT) {
                static const char *const kinds[] = { "none", "move", "press", "release", "scroll", "key", "type", "resize", "theme", "wait" };
                snprintf(last_action, sizeof last_action, "%s at %.0f,%.0f", kinds[a.k], px_, py_);
            }
            switch (a.k) {
            case A_MOVE: cui_input_mouse_move(ui, a.x, a.y); break;
            case A_DOWN: cui_input_mouse_button(ui, CUI_MOUSE_LEFT, true); break;
            case A_UP: cui_input_mouse_button(ui, CUI_MOUSE_LEFT, false); break;
            case A_SCROLL: cui_input_scroll(ui, a.x, a.y); break;
            case A_KEY: cui_input_key(ui, (cui_key)a.key, true, a.mods); cui_input_key(ui, (cui_key)a.key, false, a.mods); break;
            case A_TEXT: cui_input_key(ui, CUI_KEY_NONE, false, 0); cui_input_text(ui, a.text); break;
            case A_RESIZE: vw_ = a.x; vh_ = a.y; cui_set_viewport(ui, vw_, vh_, vw_ < 700 ? 3 : 2); break;
            case A_THEME: cui_set_theme(ui, ui->mode == CUI_THEME_DARK ? CUI_THEME_LIGHT : CUI_THEME_DARK); break;
            case A_WAIT: waiting = true; plan_at = a.at; break;
            default: break;
            }
            if (a.k != A_WAIT) plan_at = t + frand(0.15f, 1.2f);
        }
        if (opt_storm && frames % 10 == 9) {
            static const float scales[] = { 1, 1.25f, 1.5f, 2, 3 };
            vw_ = frand(320, 1600); vh_ = frand(300, 1000);
            cui_set_viewport(ui, vw_, vh_, scales[rnd() % 5]);
        }
        /* a frame, but only when the app needs one, as a real loop does */
        bool need = cui_needs_redraw(ui) || frames < 3 || (wake_at >= 0 && t >= wake_at);
        if (need) {
            double b0 = now_s();
            cui_begin_frame(ui, 1.0f / 60);
            frame(ui, user);
            cui_end_frame(ui);
            if (opt_hash) hash_draw_data(cui_get_draw_data(ui));
            double b1 = now_s();
            float bms = (float)((b1 - b0) * 1000);
            stat_add(&build, bms);
            if (t > 30) stat_add(&build_steady, bms);
            note_slow(bms, t);
            frames++;
            float wk = cui_next_wakeup(ui);
            wake_at = wk >= 0 ? t + wk : -1;   /* as a real loop sleeps until then */
            collect_targets(ui);
            /* paint every frame, incrementally; now and then check it against a full repaint */
            {
                const cui_draw_data *dd = cui_get_draw_data(ui);
                size_t need_px = (size_t)dd->width_px * dd->height_px * 4;
                if (need_px > px_cap) { free(px); free(px_check); px = malloc(need_px); px_check = malloc(need_px); px_cap = need_px; }
                double p0 = now_s();
                cui_sw_render(sw, dd, px, dd->width_px, dd->height_px, dd->width_px * 4, CUI_SW_RGBA);
                float pms = (float)((now_s() - p0) * 1000);
                stat_add(&paint, pms);
                drawn++;
                int dx, dy, dw, dh;
                bool dmg = cui_sw_last_damage(sw, &dx, &dy, &dw, &dh);
                if (dmg) painted_px += (double)dw * dh;
                char extra[24];
                snprintf(extra, sizeof extra, ", %d%% of the frame", dmg ? (int)(100.0 * dw * dh / ((double)dd->width_px * dd->height_px)) : 0);
                if (frames > 2) note_slow_in(slowest_paint, pms, t, extra);
                total_px += (double)dd->width_px * dd->height_px;
                if (frames % 1500 == 0) {
                    cui_sw_render(sw, dd, px_check, dd->width_px, dd->height_px, dd->width_px * 4, CUI_SW_RGBA);   /* another buffer: drawn whole */
                    checks++;
                    if (memcmp(px, px_check, need_px)) {
                        mismatches++;
                        if (mismatches == 1) { cui_write_png("soak-incremental.png", px, dd->width_px, dd->height_px, dd->width_px * 4); cui_write_png("soak-full.png", px_check, dd->width_px, dd->height_px, dd->width_px * 4); }
                    }
                    /* the next frame draws whole into px again (a different buffer was last) */
                }
            }
        } else idle_frames++;
        (void)waiting;
        if (t >= 60) warm = true;
        if (t >= 60 && !frames_at_warm) { live_at_warm = live_bytes; allocs_at_warm = alloc_calls; frames_at_warm = frames; }
        if (t >= 60) { if (live_bytes < min_live_after) min_live_after = live_bytes; if (live_bytes > max_live_after) max_live_after = live_bytes; }
        if (t >= next_sample) { sample(ui, t, &build, &paint, frames, drawn); next_sample += 10; }
        t += 1.0 / 60;
    }
    double wall = now_s() - t0_wall;
    long steady_frames = frames - frames_at_warm;
    printf("app %s: %.0f simulated minutes in %.0f s\n", desc && desc->title ? desc->title : "?", opt_minutes, wall);
    printf("  frames built %ld (%.0f%% of display frames; %ld skipped as idle), painted %ld\n", frames, 100.0 * frames / (frames + idle_frames), idle_frames, drawn);
    printf("  build ms: mean %.3f p50 %.3f p99 %.3f max %.2f | after warm-up p99 %.3f max %.2f\n", build.sum / CUI_MAX(1, build.n),
           stat_pct(&build, 0.5f), stat_pct(&build, 0.99f), build.max, stat_pct(&build_steady, 0.99f), build_steady.max);
    printf("  paint ms (software, incremental): mean %.2f p99 %.2f max %.2f | %.1f%% of pixels drawn | %ld checks against a full repaint, %ld differ\n",
           paint.sum / CUI_MAX(1, paint.n), stat_pct(&paint, 0.99f), paint.max, total_px > 0 ? 100 * painted_px / total_px : 0.0, checks, mismatches);
    if (!frames_at_warm) { live_at_warm = live_bytes; min_live_after = max_live_after = live_bytes; }
    printf("  library memory: %.2f MB at 1 min, %.2f MB at the end, %.2f..%.2f MB after 1 min, peak %.2f MB\n", live_at_warm / 1048576.0,
           live_bytes / 1048576.0, min_live_after / 1048576.0, max_live_after / 1048576.0, peak_bytes / 1048576.0);
    printf("  allocations after warm-up: %.3f per frame (%llu over %ld frames)\n", steady_frames ? (double)(alloc_calls - allocs_at_warm) / steady_frames : 0.0,
           alloc_calls - allocs_at_warm, steady_frames);
    if (opt_sites) { dump_sites(); dump_calls(); }
    printf("  slowest frames:");
    for (int i = 0; i < 5; i++) if (slowest[i].ms > 0) printf(" %.1f ms at %.0f s (%s);", slowest[i].ms, slowest[i].t, slowest[i].what);
    printf("\n  slowest paints:");
    for (int i = 0; i < 5; i++) if (slowest_paint[i].ms > 0) printf(" %.1f ms at %.0f s (%s);", slowest_paint[i].ms, slowest_paint[i].t, slowest_paint[i].what);
    printf("\n");
    printf("  textures: %ld live, %ld KB | states %d | process RSS %.1f MB, heap %.1f MB\n", tex_live, tex_bytes / 1024, ui->state_count, rss_mb(), heap_mb());
    printf("  person: %ld clicks, %ld typing, %ld scrolls, %ld drags, %ld keys, %ld resizes, %ld idle spells\n", n_clicks, n_types, n_scrolls, n_drags, n_keys, n_resizes, n_idles);
    cui_destroy(ui);
    printf("  after cui_destroy: %zu bytes still allocated by the library, %ld textures\n", live_bytes, tex_live);
    if (opt_hash) printf("  draw hash: %016llx\n", (unsigned long long)frame_hash);
    cui_sw_destroy(sw);
    free(px); free(px_check);
    return live_bytes != 0 || mismatches != 0;   /* a leak, or a wrong incremental frame, fails the run */
}

/* ---- windowed: a real window, real events through XTest ---- */
#if defined(__linux__) && !defined(CUI_SOAK_NO_X11)
#include <X11/Xlib.h>
#include <X11/keysym.h>
#include <pthread.h>
extern int XTestFakeMotionEvent(Display *, int, int, int, unsigned long);
extern int XTestFakeButtonEvent(Display *, unsigned int, Bool, unsigned long);
extern int XTestFakeKeyEvent(Display *, unsigned int, Bool, unsigned long);

static pthread_mutex_t mx = PTHREAD_MUTEX_INITIALIZER;
static volatile bool driving = true;
static double win_x, win_y;   /* the window's place on the screen */

static void x_key(Display *d, KeySym ks, int mods) {
    KeyCode c = XKeysymToKeycode(d, ks);
    if (!c) return;
    KeyCode ctrl = XKeysymToKeycode(d, XK_Control_L), shift = XKeysymToKeycode(d, XK_Shift_L);
    if (mods & CUI_MOD_PRIMARY) XTestFakeKeyEvent(d, ctrl, True, 0);
    if (mods & CUI_MOD_SHIFT) XTestFakeKeyEvent(d, shift, True, 0);
    XTestFakeKeyEvent(d, c, True, 0);
    XTestFakeKeyEvent(d, c, False, 0);
    if (mods & CUI_MOD_SHIFT) XTestFakeKeyEvent(d, shift, False, 0);
    if (mods & CUI_MOD_PRIMARY) XTestFakeKeyEvent(d, ctrl, False, 0);
}
static KeySym cui_to_keysym(int k) {
    switch (k) {
    case CUI_KEY_ESCAPE: return XK_Escape; case CUI_KEY_TAB: return XK_Tab; case CUI_KEY_DOWN: return XK_Down; case CUI_KEY_UP: return XK_Up;
    case CUI_KEY_LEFT: return XK_Left; case CUI_KEY_RIGHT: return XK_Right; case CUI_KEY_ENTER: return XK_Return; case CUI_KEY_SPACE: return XK_space;
    case CUI_KEY_PAGE_DOWN: return XK_Page_Down; case CUI_KEY_HOME: return XK_Home; case CUI_KEY_BACKSPACE: return XK_BackSpace; case CUI_KEY_A: return XK_a;
    default: return 0;
    }
}

static void *driver(void *arg) {
    (void)arg;
    Display *d = XOpenDisplay(NULL);
    if (!d) return NULL;
    double start = now_s(), plan_at = start + 2;
    while (driving) {
        double t = now_s();
        pthread_mutex_lock(&mx);
        if (empty() && t >= plan_at) plan(t, true);
        soak_act a = { 0 };
        bool have = !empty() && queue[qhead].at <= t;
        if (have) { a = queue[qhead]; qhead = (qhead + 1) % 256; }
        pthread_mutex_unlock(&mx);
        if (!have) { usleep(2000); continue; }
        switch (a.k) {
        case A_MOVE: XTestFakeMotionEvent(d, -1, (int)(win_x + a.x), (int)(win_y + a.y), 0); break;
        case A_DOWN: XTestFakeButtonEvent(d, 1, True, 0); break;
        case A_UP: XTestFakeButtonEvent(d, 1, False, 0); break;
        case A_SCROLL: { unsigned b = a.y > 0 ? 4 : 5; int n = (int)(fabsf(a.y) + 0.5f); for (int i = 0; i < CUI_MAX(1, n); i++) { XTestFakeButtonEvent(d, b, True, 0); XTestFakeButtonEvent(d, b, False, 0); } break; }
        case A_KEY: { KeySym ks = cui_to_keysym(a.key); if (ks) x_key(d, ks, a.mods); break; }
        case A_TEXT:
            for (const char *p = a.text; *p; p++) {
                char c = *p;
                KeySym ks = c == ' ' ? XK_space : (c >= '0' && c <= '9') ? (KeySym)(XK_0 + (c - '0')) : (c >= 'a' && c <= 'z') ? (KeySym)(XK_a + (c - 'a')) : (c >= 'A' && c <= 'Z') ? (KeySym)(XK_a + (c - 'A')) : 0;
                if (ks) x_key(d, ks, (c >= 'A' && c <= 'Z') ? CUI_MOD_SHIFT : 0);
                XFlush(d);
                usleep(30000);
            }
            break;
        case A_WAIT: pthread_mutex_lock(&mx); plan_at = a.at; pthread_mutex_unlock(&mx); break;
        default: break;   /* resizes and theme come from the app loop */
        }
        XFlush(d);
        if (a.k != A_WAIT) { pthread_mutex_lock(&mx); plan_at = now_s() + frand(0.15f, 1.2f); pthread_mutex_unlock(&mx); }
    }
    XCloseDisplay(d);
    return NULL;
}

/* idle runs: the app may sleep for good with nothing to do; one pointer move at the end wakes it */
static double idle_end;
static void *idle_waker(void *arg) {
    (void)arg;
    while (driving && now_s() < idle_end) usleep(50000);
    Display *d = XOpenDisplay(NULL);
    if (d) { XTestFakeMotionEvent(d, -1, (int)win_x + 5, (int)win_y + 5, 0); XFlush(d); XCloseDisplay(d); }
    return NULL;
}

static int run_window(const cui_app_desc *desc0, void (*frame)(cui_ctx *, void *), void *user) {
    cui_app_desc desc = desc0 ? *desc0 : (cui_app_desc){ 0 };
    desc.renderer = opt_sw ? CUI_RENDERER_SOFTWARE : CUI_RENDERER_AUTO;
    cui_app *app = cui_app_create(&desc);
    if (!app) { fprintf(stderr, "no window\n"); return 1; }
    cui_ctx *ui = cui_app_ui(app);
    if (opt_full && app->sw) cui_sw_set_incremental(app->sw, false);   /* to compare: every frame drawn whole */
    pthread_t th;
    idle_end = now_s() + opt_minutes * 60;
    pthread_create(&th, NULL, opt_idle ? idle_waker : driver, NULL);
    static soak_stat build, whole;
    double start = now_s(), end = start + opt_minutes * 60, next = start, last = start;
    t0_cpu = cpu_s(); t0_wall = start;
    long frames = 0;
    double cpu_idle_start = -1;
    double idle_cpu = 0, idle_wall = 0;
    double rss0 = 0, heap0 = 0, rss_max = 0;
    while (now_s() < end && cui_app_frame(app)) {
        double b0 = now_s();
        frame(ui, user);
        double b1 = now_s();
        stat_add(&build, (float)((b1 - b0) * 1000));
        stat_add(&whole, (float)((b1 - last) * 1000));
        last = b1;
        frames++;
        pthread_mutex_lock(&mx);
        cui_vec2 sz = cui_app_size(app);
        vw_ = sz.x; vh_ = sz.y;
        collect_targets(ui);
        bool idle_now = !empty() && queue[qhead].k == A_WAIT;
        pthread_mutex_unlock(&mx);
        (void)idle_now;
        double t = now_s();
        if (t - start > 60 && rss0 == 0) { rss0 = rss_mb(); heap0 = heap_mb(); }
        if (rss_mb() > rss_max) rss_max = rss_mb();
        if (t >= next) {
            if (csv) fprintf(csv, "%.1f,%.3f,%.3f,%ld,%d,%d,%.3f,%.1f\n", t - start, heap_mb(), rss_mb(), frames, ui->state_count, ui->node_count, build.n ? build.sum / build.n : 0.0, cpu_s() - t0_cpu), fflush(csv);
            next += 10;
        }
        (void)cpu_idle_start; (void)idle_cpu; (void)idle_wall;
    }
    driving = false;
    pthread_join(th, NULL);
    double wall = now_s() - start, cpu = cpu_s() - t0_cpu;
    printf("app %s (%s, %s): %.1f real minutes, %ld frames (%.1f per second)\n", desc.title ? desc.title : "?", cui_app_backend(app), cui_app_is_software(app) ? "software" : "OpenGL", wall / 60, frames, frames / wall);
    printf("  build ms: mean %.3f p50 %.3f p99 %.3f max %.2f\n", build.sum / CUI_MAX(1, build.n), stat_pct(&build, 0.5f), stat_pct(&build, 0.99f), build.max);
    printf("  frame to frame ms: p50 %.2f p99 %.2f max %.1f\n", stat_pct(&whole, 0.5f), stat_pct(&whole, 0.99f), whole.max);
    printf("  CPU %.1f s over %.1f s (%.1f%% of one core)\n", cpu, wall, 100 * cpu / wall);
    printf("  RSS %.1f MB at 1 min, %.1f MB at the end, max %.1f | heap %.2f MB at 1 min, %.2f MB at the end\n", rss0, rss_mb(), rss_max, heap0, heap_mb());
    printf("  person: %ld clicks, %ld typing, %ld scrolls, %ld drags, %ld keys, %ld idle spells\n", n_clicks, n_types, n_scrolls, n_drags, n_keys, n_idles);
    cui_app_destroy(app);
    return 0;
}
#else
static int run_window(const cui_app_desc *desc, void (*frame)(cui_ctx *, void *), void *user) { (void)desc; (void)frame; (void)user; fprintf(stderr, "no windowed mode here\n"); return 1; }
#endif

int soak_app_run(const cui_app_desc *desc, void (*frame)(cui_ctx *ui, void *user), void *user) {
    rng = opt_seed * 2654435761u + 1;
    if (opt_csv) csv = fopen(opt_csv, "w");
    if (csv) fprintf(csv, opt_window ? "t,heap_mb,rss_mb,frames,states,nodes,build_ms,cpu_s\n"
                                     : "t,lib_mb,heap_mb,rss_mb,allocs,textures,tex_kb,states,nodes,vertices,frames,painted,build_ms,paint_ms,cpu_s,wall_s\n");
    int r = opt_window ? run_window(desc, frame, user) : run_headless(desc, frame, user);
    if (csv) fclose(csv);
    return r;
}

int main(int argc, char **argv) {
    for (int i = 1; i < argc; i++) {
        if (!strcmp(argv[i], "--minutes") && i + 1 < argc) opt_minutes = atof(argv[++i]);
        else if (!strcmp(argv[i], "--seed") && i + 1 < argc) opt_seed = (unsigned)atoi(argv[++i]);
        else if (!strcmp(argv[i], "--window")) opt_window = true;
        else if (!strcmp(argv[i], "--sw")) opt_sw = true;
        else if (!strcmp(argv[i], "--full")) opt_full = true;
        else if (!strcmp(argv[i], "--idle")) opt_idle = true;
        else if (!strcmp(argv[i], "--storm")) opt_storm = true;   /* headless: a new window size and scale every 10 frames */   /* windowed, nobody touching it: what does waiting cost */
        else if (!strcmp(argv[i], "--hash")) opt_hash = true;
        else if (!strcmp(argv[i], "--sites")) opt_sites = true;
        else if (!strcmp(argv[i], "--deep")) opt_sites = opt_deep = true;
        else if (!strcmp(argv[i], "--csv") && i + 1 < argc) opt_csv = argv[++i];
    }
    return example_main();
}
