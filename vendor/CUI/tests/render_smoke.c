/* Renders a small screen with the software renderer and writes a PNG. */
#include <cui/cui.h>
#include <cui/cui_render.h>
#include <stdio.h>
#include <stdlib.h>

static void ui_frame(cui_ctx *ui) {
    cui_column(ui, .pad = 32, .gap = 16, .bg = CUI_COLOR_BACKGROUND, .width = CUI_FILL, .height = CUI_FILL) {
        cui_heading(ui, "Account settings", 1);
        cui_secondary(ui, "Manage your profile, notifications and billing. This paragraph wraps when the window is narrow enough to need it.");
        cui_row(ui, .gap = 8) {
            cui_button(ui, "Save changes", .variant = CUI_BUTTON_PRIMARY, .icon = CUI_ICON_CHECK);
            cui_button(ui, "Cancel");
            cui_button(ui, "Outline", .variant = CUI_BUTTON_OUTLINE);
            cui_button(ui, "Ghost", .variant = CUI_BUTTON_GHOST, .icon = CUI_ICON_SETTINGS);
            cui_button(ui, "Delete", .variant = CUI_BUTTON_DANGER, .icon = CUI_ICON_TRASH_2);
            cui_icon_button(ui, CUI_ICON_ELLIPSIS, .tooltip = "More");
        }
        cui_row(ui, .gap = 8) {
            cui_badge(ui, "Neutral");
            cui_badge(ui, "Info", .tone = CUI_TONE_INFO, .dot = true);
            cui_badge(ui, "Active", .tone = CUI_TONE_SUCCESS, .dot = true);
            cui_badge(ui, "Pending", .tone = CUI_TONE_WARNING);
            cui_badge(ui, "Failed", .tone = CUI_TONE_DANGER);
            cui_avatar(ui, "Maya Okafor", .status = "online");
            cui_avatar(ui, "Jonas Berg");
            cui_kbd(ui, "Ctrl K");
            cui_code(ui, "cui_button()");
        }
        cui_card(ui, .gap = 8) {
            cui_label(ui, "Usage");
            cui_progress(ui, 0.62f, .label = "Storage", .show_value = true);
            cui_skeleton(ui, CUI_PX(220), 12);
        }
        cui_alert(ui, "Disk almost full", .description = "Builds stop when less than 2 GB is left.", .tone = CUI_TONE_WARNING, .dismissible = true);
    }
}

int main(int argc, char **argv) {
    const char *out = argc > 1 ? argv[1] : "smoke.png";
    int dark = argc > 2 && argv[2][0] == 'd';
    cui_sw_renderer *sw = cui_sw_create();
    cui_ctx *ui = cui_create(&(cui_ctx_desc){ .renderer = cui_sw_iface(sw), .theme = dark ? CUI_THEME_DARK : CUI_THEME_LIGHT });
    float W = 900, H = 640, S = 2;
    cui_set_viewport(ui, W, H, S);
    for (int f = 0; f < 3; f++) { cui_begin_frame(ui, 1.0f / 60); ui_frame(ui); cui_end_frame(ui); }
    const cui_draw_data *dd = cui_get_draw_data(ui);
    int w = dd->width_px, h = dd->height_px;
    unsigned char *px = malloc((size_t)w * h * 4);
    cui_sw_render(sw, dd, px, w, h, w * 4, CUI_SW_RGBA);
    cui_write_png(out, px, w, h, w * 4);
    printf("vertices=%d indices=%d cmds=%d -> %s\n", dd->vertex_count, dd->index_count, dd->command_count, out);
    free(px);
    cui_destroy(ui);
    cui_sw_destroy(sw);
    return 0;
}
