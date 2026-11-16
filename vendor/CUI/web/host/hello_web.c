/* examples/hello.c with its loop body as a cui_app_run frame, so the web host can drive it */
#include <cui/cui.h>
#include <cui/cui_app.h>
typedef struct { bool notifications; int clicks; } hello_state;
static void frame(cui_ctx *ui, void *user) {
    hello_state *s = user;
    cui_column(ui, .pad = 32, .gap = 16, .width = CUI_FILL) {
        cui_heading(ui, "Hello, CUI", 2);
        cui_secondary(ui, "A native window, a few components and no dependencies.");
        cui_switch(ui, "Notifications", &s->notifications);
        cui_row(ui, .gap = 8) {
            if (cui_button(ui, "Click me", .variant = CUI_BUTTON_PRIMARY)) s->clicks++;
            cui_textf(ui, "Clicked %d times", s->clicks);
        }
    }
}
int main(void) {
    static hello_state s = { true, 0 };
    return cui_app_run(&(cui_app_desc){ .title = "Hello", .width = 420, .height = 300 }, frame, &s);
}
