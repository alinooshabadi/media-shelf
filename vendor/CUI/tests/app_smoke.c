/* Opens a real window with the native backend, runs the gallery for a few
 * frames, injects input, saves a screenshot and exits.
 *   cui_app_smoke [out.png] */
#include <cui/cui_app.h>
#include <cui/cui_render.h>
#include "../examples/gallery_ui.h"

#include <stdio.h>
#include <stdlib.h>

#define SMOKE_APP_ID "io.github.yappologistic.CUI.SmokeTest"

#ifdef CUI_SMOKE_X11
#include <X11/Xlib.h>
#include <X11/Xutil.h>
#include <string.h>
#if defined(CUI_SMOKE_SDL2)
#define SDL_MAIN_HANDLED
#include <SDL.h>
#elif defined(CUI_SMOKE_GLFW)
#include <GLFW/glfw3.h>
#endif

static int smoke_ignore_x_error(Display *dpy, XErrorEvent *e) { (void)dpy; (void)e; return 0; }

/* Walks the window tree as a window manager sees it (a manager may have
 * reparented the window into a frame) for a window whose WM_CLASS class is
 * the app ID. The X11 and GLFW backends set the instance part to the ID as
 * well, so it is compared too; with SDL2 it is not, because SDL3 under
 * sdl2-compat puts the program's name there. */
#if defined(CUI_SMOKE_SDL2)
#define SMOKE_INSTANCE_IS_ID 0
#else
#define SMOKE_INSTANCE_IS_ID 1
#endif
static bool smoke_find_wm_class(Display *dpy, Window w, int depth) {
    XClassHint ch = { 0 };
    bool found = false;
    if (XGetClassHint(dpy, w, &ch)) {
        found = ch.res_name && ch.res_class && strcmp(ch.res_class, SMOKE_APP_ID) == 0;
        if (found) printf("WM_CLASS=\"%s\", \"%s\"\n", ch.res_name, ch.res_class);
        if (found && SMOKE_INSTANCE_IS_ID && strcmp(ch.res_name, SMOKE_APP_ID) != 0) {
            printf("WM_CLASS: the instance part is not %s\n", SMOKE_APP_ID);
            found = false;
        }
        if (ch.res_name) XFree(ch.res_name);
        if (ch.res_class) XFree(ch.res_class);
    }
    Window root, parent, *kids = NULL;
    unsigned n = 0;
    if (!found && depth > 0 && XQueryTree(dpy, w, &root, &parent, &kids, &n)) {
        for (unsigned i = 0; i < n && !found; i++) found = smoke_find_wm_class(dpy, kids[i], depth - 1);
        if (kids) XFree(kids);
    }
    return found;
}

/* The window system the backend picked; only X11 has a WM_CLASS to read. */
static const char *smoke_window_system(void) {
#if defined(CUI_SMOKE_SDL2)
    const char *driver = SDL_GetCurrentVideoDriver();
    return driver ? driver : "no video driver";
#elif defined(CUI_SMOKE_GLFW) && defined(GLFW_PLATFORM_X11)   /* GLFW 3.4 */
    return glfwGetPlatform() == GLFW_PLATFORM_X11 ? "x11" : "a platform other than X11";
#elif defined(CUI_SMOKE_GLFW)   /* GLFW 3.3 cannot say which one it picked */
    return getenv("WAYLAND_DISPLAY") ? "X11 or Wayland (GLFW 3.3 does not say)" : "x11";
#else
    return "x11";
#endif
}

/* Desktops match a window to its .desktop file by WM_CLASS. */
static bool smoke_app_id_ok(cui_app *app) {
#if defined(CUI_SMOKE_SDL2)
    /* SDL2 read the ID from the environment in SDL_Init; it must not stay
     * there for child processes or a later app to inherit */
    const char *left = getenv("SDL_VIDEO_X11_WMCLASS");
    if (left && strcmp(left, SMOKE_APP_ID) == 0) {
        printf("SDL_VIDEO_X11_WMCLASS was left set to %s\n", left);
        return false;
    }
#endif
    const char *system = smoke_window_system();
    if (strcmp(system, "x11") != 0) {
        printf("WM_CLASS not checked: %s runs on %s\n", cui_app_backend(app), system);
        return true;
    }
    Display *dpy = XOpenDisplay(NULL);
    if (!dpy) return false;
    XErrorHandler old = XSetErrorHandler(smoke_ignore_x_error);   /* other clients' windows may close mid-walk */
    bool found = smoke_find_wm_class(dpy, DefaultRootWindow(dpy), 3);
    XSync(dpy, False);
    XSetErrorHandler(old);
    XCloseDisplay(dpy);
    if (!found) printf("WM_CLASS: no window carries %s\n", SMOKE_APP_ID);
    return found;
}
#endif

int main(int argc, char **argv) {
    const char *out = argc > 1 ? argv[1] : "app_smoke.png";
    setvbuf(stdout, NULL, _IONBF, 0);
    cui_app *app = cui_app_create(&(cui_app_desc){ .title = "CUI smoke test", .width = 1000, .height = 700, .continuous = true,
                                                   .app_id = SMOKE_APP_ID });
    if (!app) { fprintf(stderr, "could not create app\n"); return 1; }
    printf("backend=%s renderer=%s\n", cui_app_backend(app), cui_app_is_software(app) ? "software" : "opengl");
    static gallery g;
    gallery_init(&g);
    g.page = PAGE_SELECTION;
    int frame = 0;
    bool switched = false;
    double t0 = 0, t1 = 0;
    while (cui_app_frame(app)) {
        cui_ctx *ui = cui_app_ui(app);
        /* click the Bluetooth switch after the layout settled, wherever the gallery put it.
         * The position goes in again with the press and the release, so a real pointer
         * moving over the window in between cannot pull the click away. */
        float cx = g.bluetooth_rect.x + g.bluetooth_rect.w * 0.5f, cy = g.bluetooth_rect.y + g.bluetooth_rect.h * 0.5f;
        if (frame >= 20 && frame <= 24) cui_input_mouse_move(ui, cx, cy);
        if (frame == 22) cui_input_mouse_button(ui, CUI_MOUSE_LEFT, true);
        if (frame == 24) cui_input_mouse_button(ui, CUI_MOUSE_LEFT, false);
        bool before = g.bluetooth;
        gallery_frame(ui, &g);
        if (g.bluetooth != before) switched = true;
        if (frame % 20 == 0) printf("frame %d\n", frame);
        if (frame == 110) t0 = cui_time(ui);
        if (frame == 149) t1 = cui_time(ui);
        /* NULL names the running app, as a cui_app_run callback has to: quit that way */
        if (++frame == 150) { cui_app_set_title(NULL, "CUI smoke test, done"); cui_app_quit(NULL); }
        if (frame > 160) break;
    }
    bool quit_by_null = frame == 150;
    cui_vec2 sz = cui_app_size(app), sz_null = cui_app_size(NULL);
    bool null_ok = quit_by_null && sz.x == sz_null.x && sz.y == sz_null.y && cui_app_ui(NULL) == cui_app_ui(app);
    printf("NULL as the running app: %s\n", null_ok ? "ok" : "FAILED");
    const cui_draw_data *dd = cui_get_draw_data(cui_app_ui(app));
    int w = dd->width_px, h = dd->height_px;
    unsigned char *px = malloc((size_t)w * h * 4);
    bool ok = cui_app_screenshot(app, px, w, h) && cui_write_png(out, px, w, h, w * 4);
    printf("size=%.0fx%.0f screenshot=%s switch_clicked=%s (switch at %.0f,%.0f %.0fx%.0f)\n", sz.x, sz.y, ok ? out : "failed", switched ? "yes" : "no",
           g.bluetooth_rect.x, g.bluetooth_rect.y, g.bluetooth_rect.w, g.bluetooth_rect.h);
    free(px);
    bool app_id_ok = true;
#ifdef CUI_SMOKE_X11
    app_id_ok = smoke_app_id_ok(app);
#endif
    cui_app_destroy(app);
    /* a continuous app runs at the display's rate, not as fast as the machine can draw */
    double fps = t1 > t0 ? 39.0 / (t1 - t0) : 0;
    bool paced = fps > 0 && fps < 250;
    printf("animation: %.0f frames per second%s\n", fps, paced ? "" : " (not paced)");
    return (ok && switched && paced && null_ok && app_id_ok) ? 0 : 1;
}
