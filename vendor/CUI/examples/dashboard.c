/* Metrics dashboard: sidebar, top bar, KPIs, charts and a table.
 * Below 760 points the sidebar becomes a drawer and the panels stack. */
#include <cui/cui.h>
#include <cui/cui_app.h>

#include <stdio.h>
#include <string.h>

typedef struct {
    int nav;
    int range;
    char search[64];
    int page;
    cui_table_sort sort;
    int selected;
    bool dark;
    bool palette;
    bool drawer;     /* the navigation drawer on a narrow window */
} app_state;

static bool compact(cui_ctx *ui) { return cui_viewport_size(ui).x < 760; }

typedef struct { const char *id, *customer, *email, *date, *status; int tone; float amount; } order;

static const order orders[] = {
    { "#3021", "Nora Lindqvist", "nora@lindqvist.se", "Nov 5", "Paid", CUI_TONE_SUCCESS, 184.20f },
    { "#3020", "Kwame Asante", "kwame@asante.co", "Nov 5", "Pending", CUI_TONE_WARNING, 42.75f },
    { "#3019", "Hana Novak", "hana@novak.cz", "Nov 4", "Paid", CUI_TONE_SUCCESS, 318.40f },
    { "#3018", "Felix Moreau", "felix@moreau.fr", "Nov 4", "Refunded", CUI_TONE_DANGER, 96.00f },
    { "#3017", "Iris Coleman", "iris@coleman.org", "Nov 3", "Paid", CUI_TONE_SUCCESS, 471.15f },
    { "#3016", "Ravi Shah", "ravi@shah.in", "Nov 3", "Paid", CUI_TONE_SUCCESS, 1236.80f },
};

static void nav_items(cui_ctx *ui, app_state *s) {
    static const char *items[] = { "Overview", "Orders", "Customers", "Products", "Analytics", "Settings" };
    static const char *icons[] = { CUI_ICON_LAYOUT_DASHBOARD, CUI_ICON_SHOPPING_CART, CUI_ICON_USERS, CUI_ICON_PACKAGE,
                                   CUI_ICON_CHART_BAR, CUI_ICON_SETTINGS };
    static const char *badges[] = { NULL, "12", NULL, NULL, NULL, NULL };
    for (int i = 0; i < 6; i++)
        if (cui_nav_item(ui, items[i], .icon = icons[i], .selected = s->nav == i, .badge = badges[i])) { s->nav = i; s->drawer = false; }
}

static void sidebar(cui_ctx *ui, app_state *s) {
    cui_column(ui, .width = CUI_PX(232), .height = CUI_FILL, .bg = CUI_COLOR_SURFACE_MUTED, .pad = 12, .gap = 2) {
        cui_row(ui, .gap = 10, .pad_x = 8, .pad_top = 6, .pad_bottom = 16) {
            cui_box(ui, .width = CUI_PX(26), .height = CUI_PX(26), .radius = 7, .bg = CUI_COLOR_FOREGROUND, .dir = CUI_STACK,
                    .align = CUI_ALIGN_CENTER, .justify = CUI_JUSTIFY_CENTER) {
                cui_text(ui, "L", .font = CUI_FONT_DISPLAY, .size = 14, .color = CUI_COLOR_BACKGROUND);
            }
            cui_text(ui, "Larkspur Supply", .font = CUI_FONT_MEDIUM, .size = 15, .nowrap = true);
        }
        nav_items(ui, s);
        cui_spacer(ui);
        cui_card(ui, .pad = 16, .gap = 10) {
            cui_text(ui, "Next payout", .size = 12, .color = CUI_COLOR_TEXT_SECONDARY);
            cui_text(ui, "$4,812.30", .font = CUI_FONT_DISPLAY, .size = 20, .tabular = true);
            cui_text(ui, "To the account ending 4417 on Nov 8.", .size = 12, .color = CUI_COLOR_TEXT_SECONDARY);
        }
    }
}

static void topbar(cui_ctx *ui, app_state *s) {
    bool narrow = compact(ui);
    cui_row(ui, .gap = narrow ? 8 : 12, .pad_x = narrow ? 12 : 32, .pad_y = 14, .width = CUI_FILL) {
        if (narrow && cui_icon_button(ui, CUI_ICON_MENU, .tooltip = "Menu")) s->drawer = true;
        cui_box(ui, .width = CUI_GROW(4), .max_width = 320) {
            cui_search(ui, "search", s->search, sizeof s->search, .placeholder = "Search orders, customers");
        }
        cui_spacer(ui);
        if (cui_icon_button(ui, cui_get_theme_mode(ui) == CUI_THEME_DARK ? CUI_ICON_SUN : CUI_ICON_MOON, .tooltip = "Toggle theme")) {
            s->dark = cui_get_theme_mode(ui) != CUI_THEME_DARK;
            cui_set_theme(ui, s->dark ? CUI_THEME_DARK : CUI_THEME_LIGHT);
        }
        cui_icon_button(ui, CUI_ICON_BELL, .tooltip = "Notifications");
        cui_menu(ui, "Account", .icon = CUI_ICON_CIRCLE_USER, .variant = CUI_BUTTON_GHOST, .placement = CUI_PLACE_BOTTOM_END) {
            cui_menu_label(ui, "nora@larkspur.co");
            cui_menu_item(ui, "Profile", .icon = CUI_ICON_USER);
            cui_menu_item(ui, "Billing", .icon = CUI_ICON_CREDIT_CARD);
            cui_menu_item(ui, "Keyboard shortcuts", .icon = CUI_ICON_KEYBOARD, .shortcut = "?");
            cui_menu_separator(ui);
            cui_menu_item(ui, "Log out", .icon = CUI_ICON_LOG_OUT);
        }
    }
}

static void kpi(cui_ctx *ui, const char *label, const char *value, const char *change, bool good) {
    cui_column(ui, .width = CUI_GROW(1), .pad_x = 22, .gap = 6) {
        cui_text(ui, label, .size = 13, .color = CUI_COLOR_TEXT_SECONDARY);
        cui_number_roll(ui, label, value, .font = CUI_FONT_DISPLAY, .size = 26);   /* the figures roll when the range changes */
        cui_text(ui, change, .size = 12, .color = good ? CUI_COLOR_SUCCESS : CUI_COLOR_DANGER);
    }
}

static void content(cui_ctx *ui, app_state *s) {
    static const float rev[] = { 31, 34, 33, 38, 42, 40, 46, 52, 49, 55, 61, 64 };
    static const char *months[] = { "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec" };

    bool narrow = compact(ui);
    cui_row(ui, .gap = 12, .wrap = true, .width = CUI_FILL) {
        cui_column(ui, .gap = 4, .width = CUI_FILL, .min_width = 220) {
            cui_heading(ui, "Overview", 2);
            cui_secondary(ui, "November 1 to 30, all channels");
        }
        const char *ranges[] = { "7 days", "30 days", "12 months" };
        cui_segmented(ui, "range", ranges, 3, &s->range, .size = CUI_SIZE_SM);
        cui_button(ui, "Export", .icon = CUI_ICON_DOWNLOAD, .size = CUI_SIZE_SM);
    }
    /* one strip of figures rather than four look-alike cards; the chart below carries the trend */
    cui_card(ui, .dir = CUI_ROW, .gap = 0, .pad = 0) {
        static const char *rev[] = { "$15,830", "$64,210", "$702,480" }, *ord[] = { "318", "1,286", "14,903" };
        static const char *conv[] = { "4.3%", "4.1%", "3.8%" }, *ref[] = { "1.5%", "1.7%", "2.0%" };
        static const char *since[] = { "on last week", "on October", "on last year" };
        int k = s->range;
        char c1[48], c2[48];
        snprintf(c1, sizeof c1, "%s %s", k == 2 ? "+31.0%" : k ? "+12.4%" : "+3.1%", since[k]);
        snprintf(c2, sizeof c2, "%s %s", k == 2 ? "+22.6%" : k ? "+7.3%" : "+1.9%", since[k]);
        /* without room for four figures in a strip, two per line and no hairlines */
        float room = cui_viewport_size(ui).x - (narrow ? 32 : 297);
        bool two = room < 720;
        cui_box(ui, .dir = two ? CUI_GRID : CUI_ROW, .columns = 2, .width = CUI_FILL, .gap = two ? 20 : 0, .pad_y = two ? 4 : 0) {
            kpi(ui, "Revenue", rev[k], c1, true);
            if (!two) cui_separator(ui);
            kpi(ui, "Orders", ord[k], c2, true);
            if (!two) cui_separator(ui);
            kpi(ui, "Conversion", conv[k], k == 2 ? "+0.6 pts" : k ? "+0.4 pts" : "+0.2 pts", true);
            if (!two) cui_separator(ui);
            kpi(ui, "Refund rate", ref[k], k == 2 ? "-0.3 pts, fewer returns" : k ? "-0.6 pts, fewer returns" : "-0.2 pts, fewer returns", true);
        }
    }
    cui_row(ui, .gap = 16, .align = CUI_ALIGN_STRETCH, .wrap = true, .width = CUI_FILL) {
        cui_card(ui, .gap = 16, .width = CUI_GROW(2), .min_width = 300) {
            cui_row(ui) {
                cui_column(ui, .gap = 2, .width = CUI_FILL) {
                    cui_label(ui, "Revenue");
                    cui_text(ui, "Compared with last year", .size = 12, .color = CUI_COLOR_TEXT_SECONDARY);
                }
                cui_badge(ui, "+18%", .tone = CUI_TONE_SUCCESS, .icon = CUI_ICON_TRENDING_UP);
            }
            static const float last[] = { 24, 26, 27, 29, 31, 30, 34, 36, 35, 39, 41, 44 };
            cui_series series[2] = { { "This year", rev, 12, { 0 } }, { "Last year", last, 12, { 0 } } };
            cui_line_chart(ui, "rev", series, 2, .labels = months, .label_count = 12, .area = true, .smooth = true,
                           .height = 230, .format = "$%.0fk");
        }
        cui_card(ui, .gap = 16, .width = CUI_GROW(1), .min_width = 260) {
            cui_column(ui, .gap = 2) {
                cui_label(ui, "Where orders come from");
                cui_text(ui, "Share of November orders", .size = 12, .color = CUI_COLOR_TEXT_SECONDARY);
            }
            static const float src[] = { 42, 26, 18, 14 };
            static const char *names[] = { "Search", "Direct", "Social", "Email" };
            cui_column(ui, .width = CUI_FILL, .height = CUI_FILL, .justify = CUI_JUSTIFY_CENTER) {
                cui_donut_chart(ui, "src", src, names, 4, .height = 170, .format = "%.0f%%");
            }
        }
    }
    cui_card(ui, .gap = 14) {
        cui_row(ui) {
            cui_column(ui, .gap = 2, .width = CUI_FILL) {
                cui_label(ui, "Recent orders");
                cui_text(ui, "Newest first", .size = 12, .color = CUI_COLOR_TEXT_SECONDARY);
            }
            cui_button(ui, "View all", .variant = CUI_BUTTON_GHOST, .icon_right = CUI_ICON_ARROW_RIGHT, .size = CUI_SIZE_SM);
        }
        cui_table_column cols[] = {
            { "Order", CUI_PX(90), CUI_TEXT_LEFT, true },
            { "Customer", CUI_FILL, CUI_TEXT_LEFT, true },
            { "Date", CUI_PX(90), CUI_TEXT_LEFT, false },
            { "Status", CUI_PX(120), CUI_TEXT_LEFT, true },
            { "Amount", CUI_PX(110), CUI_TEXT_RIGHT, true },
        };
        cui_table(ui, "orders", cols, 5, .sort = &s->sort, .selectable = true) {
            for (int i = 0; i < 6; i++) {
                const order *o = &orders[i];
                if (s->search[0] && !strstr(o->customer, s->search) && !strstr(o->id, s->search)) continue;
                if (cui_table_row(ui, s->selected == i)) s->selected = i;
                cui_table_cell_text(ui, o->id);
                cui_table_cell(ui);
                cui_avatar(ui, o->customer, .size = 28);
                cui_column(ui, .gap = 0, .width = CUI_FILL) {
                    cui_text(ui, o->customer, .font = CUI_FONT_MEDIUM, .nowrap = true);
                    cui_text(ui, o->email, .size = 12, .color = CUI_COLOR_TEXT_SECONDARY, .nowrap = true);
                }
                cui_table_cell_text(ui, o->date);
                cui_table_cell(ui);
                cui_badge(ui, o->status, .tone = (cui_tone)o->tone, .dot = true);
                cui_table_cell_textf(ui, "$%.2f", (double)o->amount);
            }
        }
        cui_row(ui, .wrap = true, .gap = 8, .width = CUI_FILL) {
            cui_box(ui, .width = CUI_FILL, .min_width = 200) { cui_text(ui, "Showing 1 to 6 of 1,286 orders", .size = 12, .color = CUI_COLOR_TEXT_SECONDARY); }
            cui_pagination(ui, "pages", &s->page, 215);
        }
    }
}

static void frame(cui_ctx *ui, void *user) {
    app_state *s = user;
    if (cui_shortcut(ui, CUI_MOD_PRIMARY, CUI_KEY_K)) s->palette = true;
    bool narrow = compact(ui);
    if (!narrow) s->drawer = false;
    cui_row(ui, .width = CUI_FILL, .height = CUI_FILL, .align = CUI_ALIGN_STRETCH, .bg = CUI_COLOR_BACKGROUND) {
        if (!narrow) { sidebar(ui, s); cui_separator(ui); }
        cui_column(ui, .width = CUI_FILL) {
            topbar(ui, s);
            cui_separator(ui);
            cui_scroll(ui, .id = "content", .pad = narrow ? 16 : 32, .gap = narrow ? 16 : 24, .fade_edges = true) {
                content(ui, s);
            }
        }
    }
    cui_sheet(ui, "Larkspur Supply", &s->drawer, .side = CUI_SIDE_LEFT, .size = 280) { nav_items(ui, s); }
    static const cui_command cmds[] = {
        { "Create order", CUI_ICON_PLUS, "Actions", "Ctrl N" },
        { "Add customer", CUI_ICON_USER_PLUS, "Actions", NULL },
        { "Export report", CUI_ICON_DOWNLOAD, "Actions", NULL },
        { "Go to analytics", CUI_ICON_CHART_BAR, "Navigation", NULL },
    };
    int c = cui_command_palette(ui, &s->palette, cmds, 4);
    if (c >= 0) cui_toast(ui, cmds[c].title, .tone = CUI_TONE_SUCCESS);
}

int main(void) {
    static app_state s = { .sort = { 0, true }, .selected = 0, .range = 1 };
    return cui_app_run(&(cui_app_desc){ .title = "Larkspur Dashboard", .width = 1360, .height = 900, .accent = CUI_ACCENT_BLUE }, frame, &s);
}
