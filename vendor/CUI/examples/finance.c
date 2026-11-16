/* Personal finance: top navigation, a balance chart, a painted payment card,
 * budgets, savings goals and recent transactions. Dark by default. The cards
 * wrap onto new lines as the window narrows; on a phone the tabs get a line
 * of their own. */
#include <cui/cui.h>
#include <cui/cui_app.h>

#include <stdio.h>
#include <string.h>

typedef struct {
    int tab, range, card;
    bool frozen, palette;
    char search[64];
} app_state;

typedef struct { const char *merchant, *category, *icon, *when; float amount; cui_tone tone; } txn;
static const txn txns[] = {
    { "Salary", "Income", CUI_ICON_BRIEFCASE, "Today, 09:12", 6200.00f, CUI_TONE_SUCCESS },
    { "Blue Bottle", "Coffee", CUI_ICON_GIFT, "Today, 08:40", -6.50f, CUI_TONE_WARNING },
    { "Fieldnotes Rent", "Housing", CUI_ICON_BUILDING_2, "Yesterday", -1850.00f, CUI_TONE_INFO },
    { "Night Market", "Groceries", CUI_ICON_SHOPPING_CART, "Yesterday", -84.20f, CUI_TONE_TEAL },
    { "Cloud Storage", "Subscriptions", CUI_ICON_CLOUD, "Nov 3", -9.99f, CUI_TONE_VIOLET },
    { "Transfer from Sam", "Transfer", CUI_ICON_ARROW_DOWN, "Nov 2", 120.00f, CUI_TONE_SUCCESS },
};
#define TXN_COUNT ((int)(sizeof txns / sizeof txns[0]))

/* --- the payment card ---------------------------------------------------- */

typedef struct { bool frozen; } card_paint;
static void paint_card(cui_ctx *ui, cui_rect r, void *user) {
    card_paint *c = user;
    cui_corners rad = CUI_CORNERS(18);
    cui_draw_shadow(ui, r, rad, 32, -6, (cui_vec2){ 0, 16 }, CUI_RGBA(0x00000080));
    cui_draw_rect_gradient(ui, r, rad, CUI_RGB(0x1f6f5c), CUI_RGB(0x0b2a24));
    cui_push_clip(ui, r);
    /* printed rings, like the guilloche on a real card; no glow */
    cui_draw_arc(ui, (cui_vec2){ r.x + r.w * 0.92f, r.y + r.h * 1.05f }, r.h * 0.75f, 0, 6.2831853f, 1, CUI_RGBA(0xffffff1c));
    cui_draw_arc(ui, (cui_vec2){ r.x + r.w * 0.92f, r.y + r.h * 1.05f }, r.h * 0.55f, 0, 6.2831853f, 1, CUI_RGBA(0xffffff14));
    cui_pop_clip(ui);
    cui_draw_rect_border(ui, r, rad, CUI_TRANSPARENT, CUI_RGBA(0xffffff22), 1);

    cui_color white = CUI_RGB(0xffffff), soft = CUI_RGBA(0xffffffb0);
    cui_draw_text(ui, CUI_FONT_DISPLAY, 17, (cui_vec2){ r.x + 24, r.y + 22 }, white, "Halden", -1);
    cui_draw_icon(ui, CUI_ICON_WIFI, (cui_rect){ r.x + r.w - 48, r.y + 16, 28, 28 }, 18, soft);
    /* chip */
    cui_rect chip = { r.x + 24, r.y + r.h * 0.40f, 40, 30 };
    cui_draw_rect_gradient(ui, chip, CUI_CORNERS(6), CUI_RGB(0xf3d58a), CUI_RGB(0xb8913f));
    cui_draw_line(ui, (cui_vec2){ chip.x, chip.y + 15 }, (cui_vec2){ chip.x + 40, chip.y + 15 }, 1, CUI_RGBA(0x00000040));
    cui_draw_line(ui, (cui_vec2){ chip.x + 14, chip.y }, (cui_vec2){ chip.x + 14, chip.y + 30 }, 1, CUI_RGBA(0x00000040));
    cui_draw_line(ui, (cui_vec2){ chip.x + 26, chip.y }, (cui_vec2){ chip.x + 26, chip.y + 30 }, 1, CUI_RGBA(0x00000040));
    cui_draw_text(ui, CUI_FONT_MONO, 17, (cui_vec2){ r.x + 24, r.y + r.h - 64 }, white, "4821  ••••  ••••  0937", -1);
    cui_draw_text(ui, CUI_FONT_BODY, 12, (cui_vec2){ r.x + 24, r.y + r.h - 34 }, soft, "MARA QUINN", -1);
    cui_draw_text(ui, CUI_FONT_BODY, 12, (cui_vec2){ r.x + r.w * 0.5f, r.y + r.h - 34 }, soft, "09 / 29", -1);
    cui_draw_text(ui, CUI_FONT_MEDIUM, 13, (cui_vec2){ r.x + r.w - 66, r.y + r.h - 40 }, white, "DEBIT", -1);
    if (c->frozen) {
        cui_draw_rect(ui, r, rad, CUI_RGBA(0x0a0f0ec0));
        cui_draw_icon(ui, CUI_ICON_LOCK, r, 28, white);
    }
}

/* --- pieces --------------------------------------------------------------- */

static bool compact(cui_ctx *ui) { return cui_viewport_size(ui).x < 900; }

static void topbar(cui_ctx *ui, app_state *s) {
    static const char *tabs[] = { "Overview", "Accounts", "Payments", "Cards", "Insights" };
    bool narrow = compact(ui);
    cui_row(ui, .width = CUI_FILL, .pad_x = narrow ? 16 : 32, .pad_top = 14, .gap = narrow ? 12 : 28, .align = CUI_ALIGN_CENTER) {
        cui_row(ui, .gap = 10, .align = CUI_ALIGN_CENTER) {
            cui_box(ui, .width = CUI_PX(26), .height = CUI_PX(26), .radius = 13, .bg = CUI_COLOR_ACCENT, .dir = CUI_STACK,
                    .align = CUI_ALIGN_CENTER, .justify = CUI_JUSTIFY_CENTER) {
                cui_text(ui, "h", .font = CUI_FONT_DISPLAY, .size = 15, .color = CUI_COLOR_ACCENT_FOREGROUND);
            }
            cui_text(ui, "Halden", .font = CUI_FONT_DISPLAY, .size = 17);
        }
        if (!narrow) cui_tabs(ui, "nav", tabs, 5, &s->tab);
        cui_spacer(ui);
        cui_box(ui, .width = CUI_GROW(4), .max_width = 220) { cui_search(ui, "search", s->search, sizeof s->search, .placeholder = "Search"); }
        cui_icon_button(ui, CUI_ICON_BELL, .tooltip = "Notifications");
        cui_avatar(ui, "Mara Quinn", .size = 32, .status = "online");
    }
    if (narrow) cui_box(ui, .width = CUI_FILL, .pad_x = 16, .pad_top = 6) { cui_tabs(ui, "nav", tabs, 5, &s->tab); }
}

static void balance_card(cui_ctx *ui, app_state *s) {
    static const char *months[] = { "Dec", "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov" };
    static const float balance[] = { 31.2f, 32.8f, 32.1f, 34.6f, 36.0f, 35.2f, 38.4f, 40.1f, 39.3f, 42.8f, 45.1f, 48.2f };
    static const float spent[] = { 4.1f, 3.8f, 4.4f, 3.9f, 4.2f, 4.6f, 3.7f, 4.0f, 4.5f, 3.6f, 3.9f, 3.4f };
    cui_card(ui, .gap = 18, .width = CUI_GROW(2), .min_width = 300) {
        cui_row(ui, .width = CUI_FILL, .align = CUI_ALIGN_START, .wrap = true, .gap = 12) {
            cui_column(ui, .gap = 6, .width = CUI_FILL, .min_width = 240) {
                cui_text(ui, "Total balance", .size = 13, .color = CUI_COLOR_TEXT_SECONDARY);
                cui_row(ui, .gap = 12, .align = CUI_ALIGN_CENTER) {
                    cui_text(ui, "$48,250.32", .font = CUI_FONT_DISPLAY, .size = 38, .tabular = true);
                    cui_badge(ui, "+6.9%", .tone = CUI_TONE_SUCCESS, .icon = CUI_ICON_TRENDING_UP);
                }
                cui_text(ui, "+$3,104 since last month across 4 accounts", .size = 13, .color = CUI_COLOR_TEXT_MUTED);
            }
            const char *ranges[] = { "1M", "6M", "1Y", "All" };
            cui_segmented(ui, "range", ranges, 4, &s->range, .size = CUI_SIZE_SM);
        }
        cui_series series[2] = { { "Balance", balance, 12, { 0 } }, { "Spending", spent, 12, { 0 } } };
        cui_line_chart(ui, "balance", series, 1, .labels = months, .label_count = 12, .area = true, .smooth = true,
                       .height = 236, .format = "$%.0fk", .no_legend = true);
        (void)series[1];
    }
}

static void card_panel(cui_ctx *ui, app_state *s) {
    cui_card(ui, .gap = 18, .width = CUI_GROW(1), .min_width = 280) {
        cui_row(ui, .width = CUI_FILL, .align = CUI_ALIGN_CENTER) {
            cui_label(ui, "My cards");
            cui_spacer(ui);
            cui_button(ui, "Add", .icon = CUI_ICON_PLUS, .size = CUI_SIZE_SM, .variant = CUI_BUTTON_GHOST);
        }
        card_paint c = { s->frozen };
        cui_canvas(ui, "card", CUI_FILL, CUI_PX(200), paint_card, &c, sizeof c);
        cui_box(ui, .dir = CUI_GRID, .columns = 3, .gap = 10, .width = CUI_FILL) {
            static const char *labels[] = { "Send", "Request", "Top up" };
            static const char *icons[] = { CUI_ICON_ARROW_UP_RIGHT, CUI_ICON_ARROW_DOWN, CUI_ICON_PLUS };
            for (int i = 0; i < 3; i++)
                cui_button(ui, labels[i], .icon = icons[i], .full_width = true);
        }
        cui_row(ui, .width = CUI_FILL, .gap = 12, .align = CUI_ALIGN_CENTER) {
            cui_column(ui, .gap = 2, .width = CUI_FILL) {
                cui_text(ui, "Freeze card", .font = CUI_FONT_MEDIUM, .size = 13);
                cui_text(ui, "Block new payments instantly", .size = 12, .color = CUI_COLOR_TEXT_MUTED);
            }
            cui_switch(ui, "##freeze", &s->frozen);
        }
    }
}

static void spending_card(cui_ctx *ui) {
    cui_card(ui, .gap = 20, .width = CUI_GROW(1), .min_width = 260) {
        cui_row(ui, .width = CUI_FILL, .align = CUI_ALIGN_CENTER) {
            cui_column(ui, .gap = 2, .width = CUI_FILL) {
                cui_label(ui, "Spending");
                cui_text(ui, "November, by category", .size = 12, .color = CUI_COLOR_TEXT_SECONDARY);
            }
            cui_menu(ui, "Options", .icon = CUI_ICON_ELLIPSIS, .icon_only = true, .variant = CUI_BUTTON_GHOST, .placement = CUI_PLACE_BOTTOM_END) {
                cui_menu_item(ui, "Export CSV", .icon = CUI_ICON_DOWNLOAD);
                cui_menu_item(ui, "Edit categories", .icon = CUI_ICON_TAG);
            }
        }
        static const float vals[] = { 1850, 640, 420, 310 };
        static const char *names[] = { "Housing", "Groceries", "Dining", "Other" };
        cui_donut_chart(ui, "spend", vals, names, 4, .height = 150, .format = "$%.0f");
    }
}

static void budgets_card(cui_ctx *ui) {
    typedef struct { const char *name, *icon; float used, total; } budget;
    static const budget b[] = {
        { "Groceries", CUI_ICON_SHOPPING_CART, 640, 800 },
        { "Dining out", CUI_ICON_GIFT, 420, 400 },
        { "Transport", CUI_ICON_MAP_PIN, 96, 250 },
        { "Entertainment", CUI_ICON_MUSIC, 186, 200 },
    };
    cui_card(ui, .gap = 16, .width = CUI_GROW(1), .min_width = 260) {
        cui_row(ui, .width = CUI_FILL, .align = CUI_ALIGN_CENTER) {
            cui_column(ui, .gap = 2, .width = CUI_FILL) {
                cui_label(ui, "Budgets");
                cui_text(ui, "12 days left this month", .size = 12, .color = CUI_COLOR_TEXT_SECONDARY);
            }
            cui_button(ui, "Edit", .size = CUI_SIZE_SM, .variant = CUI_BUTTON_GHOST);
        }
        for (int i = 0; i < 4; i++) {
            cui_push_id_int(ui, i);
            cui_column(ui, .gap = 8, .width = CUI_FILL) {
                cui_row(ui, .width = CUI_FILL, .gap = 8, .align = CUI_ALIGN_CENTER) {
                    cui_icon(ui, b[i].icon, 14, CUI_COLOR_TEXT_SECONDARY);
                    cui_text(ui, b[i].name, .size = 13, .width = CUI_FILL, .nowrap = true);
                    char t[48];
                    snprintf(t, sizeof t, "$%.0f / $%.0f", (double)b[i].used, (double)b[i].total);
                    cui_text(ui, t, .size = 12, .tabular = true,
                             .color = b[i].used > b[i].total ? CUI_COLOR_DANGER : CUI_COLOR_TEXT_SECONDARY);
                }
                /* one rule for every budget: over is red, the last tenth is amber, the rest is plain */
                float v = b[i].used / b[i].total;
                cui_tone tone = v > 1 ? CUI_TONE_DANGER : v >= 0.9f ? CUI_TONE_WARNING : CUI_TONE_NEUTRAL;
                cui_progress(ui, v > 1 ? 1 : v, .height = 6, .tone = tone);
            }
            cui_pop_id(ui);
        }
    }
}

static void goals_card(cui_ctx *ui) {
    cui_card(ui, .gap = 18, .width = CUI_GROW(1), .min_width = 260) {
        cui_row(ui, .width = CUI_FILL, .align = CUI_ALIGN_CENTER) {
            cui_column(ui, .gap = 2, .width = CUI_FILL) {
                cui_label(ui, "Savings goals");
                cui_text(ui, "On track for 2 of 3", .size = 12, .color = CUI_COLOR_TEXT_SECONDARY);
            }
            cui_icon_button(ui, CUI_ICON_PLUS, .size = CUI_SIZE_SM, .tooltip = "New goal");
        }
        cui_row(ui, .width = CUI_FILL, .justify = CUI_JUSTIFY_BETWEEN) {
            static const char *names[] = { "Japan trip", "New laptop", "Emergency" };
            static const char *amts[] = { "$3,240 of $4,000", "$1,100 of $2,400", "$9,800 of $10k" };
            static const float v[] = { 0.81f, 0.46f, 0.98f };
            for (int i = 0; i < 3; i++) {
                cui_push_id_int(ui, i);
                cui_column(ui, .gap = 8, .align = CUI_ALIGN_CENTER) {
                    cui_progress_ring(ui, "##goal", v[i], 72);
                    cui_text(ui, names[i], .font = CUI_FONT_MEDIUM, .size = 13);
                    cui_text(ui, amts[i], .size = 11, .color = CUI_COLOR_TEXT_MUTED);
                }
                cui_pop_id(ui);
            }
        }
    }
}

static void tone_icon(cui_ctx *ui, const char *icon, cui_tone tone) {
    static const cui_token tok[] = { CUI_TOKEN_TEXT_SECONDARY, CUI_TOKEN_INFO, CUI_TOKEN_SUCCESS, CUI_TOKEN_WARNING, CUI_TOKEN_DANGER,
                                     CUI_TOKEN_SERIES_2, CUI_TOKEN_SERIES_3, CUI_TOKEN_SERIES_4, CUI_TOKEN_ACCENT };
    cui_color c = cui_token_color(ui, tok[tone]);
    cui_box(ui, .width = CUI_PX(38), .height = CUI_PX(38), .radius = 12, .bg = cui_alpha(c, 0.14f), .dir = CUI_STACK,
            .align = CUI_ALIGN_CENTER, .justify = CUI_JUSTIFY_CENTER) {
        cui_icon(ui, icon, 16, c);
    }
}

static void transactions_card(cui_ctx *ui) {
    cui_card(ui, .gap = 6, .width = CUI_GROW(2)) {
        cui_row(ui, .width = CUI_FILL, .pad_bottom = 8, .align = CUI_ALIGN_CENTER) {
            cui_column(ui, .gap = 2, .width = CUI_FILL) {
                cui_label(ui, "Recent transactions");
                cui_text(ui, "Across all accounts", .size = 12, .color = CUI_COLOR_TEXT_SECONDARY);
            }
            cui_button(ui, "View all", .variant = CUI_BUTTON_GHOST, .size = CUI_SIZE_SM, .icon_right = CUI_ICON_ARROW_RIGHT);
        }
        for (int i = 0; i < TXN_COUNT; i++) {
            const txn *t = &txns[i];
            cui_push_id_int(ui, i);
            cui_row(ui, .width = CUI_FILL, .gap = 14, .pad_y = 7, .align = CUI_ALIGN_CENTER) {
                tone_icon(ui, t->icon, t->tone);
                cui_column(ui, .gap = 1, .width = CUI_FILL) {
                    cui_text(ui, t->merchant, .font = CUI_FONT_MEDIUM, .nowrap = true);
                    cui_text(ui, t->when, .size = 12, .color = CUI_COLOR_TEXT_MUTED, .nowrap = true);
                }
                if (cui_viewport_size(ui).x >= 560) cui_badge(ui, t->category);   /* a phone keeps the merchant and the amount */
                char a[32];
                snprintf(a, sizeof a, "%s$%.2f", t->amount < 0 ? "-" : "+", (double)(t->amount < 0 ? -t->amount : t->amount));
                cui_text(ui, a, .font = CUI_FONT_MEDIUM, .tabular = true, .width = CUI_PX(110), .align = CUI_TEXT_RIGHT,
                         .color = t->amount > 0 ? CUI_COLOR_SUCCESS : CUI_COLOR_FOREGROUND);
            }
            cui_pop_id(ui);
        }
    }
}

static void frame(cui_ctx *ui, void *user) {
    app_state *s = user;
    if (cui_shortcut(ui, CUI_MOD_PRIMARY, CUI_KEY_K)) s->palette = true;
    cui_column(ui, .width = CUI_FILL, .height = CUI_FILL, .bg = CUI_COLOR_BACKGROUND) {
        topbar(ui, s);
        cui_separator(ui);
        bool narrow = compact(ui);
        cui_scroll(ui, .id = "main", .pad = narrow ? 16 : 32, .gap = narrow ? 16 : 20, .fade_edges = true) {
            cui_row(ui, .width = CUI_FILL, .align = CUI_ALIGN_END, .wrap = true, .gap = 12) {
                cui_column(ui, .gap = 4, .width = CUI_FILL, .min_width = 240) {
                    cui_text(ui, "Overview", .font = CUI_FONT_DISPLAY, .size = 28);
                    cui_text(ui, "Four accounts, synced at 6:42 pm", .color = CUI_COLOR_TEXT_SECONDARY);
                }
                cui_row(ui, .gap = 8) {
                    cui_button(ui, "Statements", .icon = CUI_ICON_FILE_TEXT, .variant = CUI_BUTTON_OUTLINE);
                    cui_button(ui, "Move money", .icon = CUI_ICON_SEND, .variant = CUI_BUTTON_PRIMARY);
                }
            }
            cui_row(ui, .width = CUI_FILL, .gap = narrow ? 16 : 20, .align = CUI_ALIGN_STRETCH, .wrap = true) {
                balance_card(ui, s);
                card_panel(ui, s);
            }
            cui_row(ui, .width = CUI_FILL, .gap = narrow ? 16 : 20, .align = CUI_ALIGN_STRETCH, .wrap = true) {
                spending_card(ui);
                budgets_card(ui);
                goals_card(ui);
            }
            cui_row(ui, .width = CUI_FILL, .gap = 20, .align = CUI_ALIGN_STRETCH) {
                transactions_card(ui);
            }
        }
    }
    static const cui_command cmds[] = {
        { "Send money", CUI_ICON_SEND, "Actions", NULL },
        { "Freeze card", CUI_ICON_LOCK, "Actions", NULL },
        { "Download statement", CUI_ICON_DOWNLOAD, "Actions", NULL },
    };
    int c = cui_command_palette(ui, &s->palette, cmds, 3);
    if (c == 1) s->frozen = !s->frozen;
    else if (c >= 0) cui_toast(ui, cmds[c].title, .tone = CUI_TONE_SUCCESS);
}

int main(void) {
    static app_state s = { .range = 2 };
    return cui_app_run(&(cui_app_desc){ .title = "Halden", .width = 1440, .height = 940,
                                        .theme = CUI_THEME_DARK, .accent = CUI_ACCENT_GREEN }, frame, &s);
}
