/* Issue tracker: grouped issue list, custom status and priority glyphs,
 * a detail pane with properties and activity. Dark by default. Narrower
 * windows open the detail in a sheet, and a phone keeps the sidebar in a
 * drawer. */
#include <cui/cui.h>
#include <cui/cui_app.h>

#include <math.h>
#include <stdio.h>
#include <string.h>

enum { ST_BACKLOG, ST_TODO, ST_PROGRESS, ST_REVIEW, ST_DONE, ST_COUNT };
enum { PRI_NONE, PRI_LOW, PRI_MEDIUM, PRI_HIGH, PRI_URGENT };

typedef struct {
    const char *key, *title, *assignee, *date;
    int status, priority;
    const char *labels[2];
    cui_tone label_tones[2];
} issue;

static issue issues[] = {
    { "ENG-214", "Text input drops the last character on fast typing", "Maya Okafor", "Nov 6", ST_PROGRESS, PRI_URGENT, { "Bug", "Input" }, { CUI_TONE_DANGER, CUI_TONE_NEUTRAL } },
    { "ENG-209", "Spring animations for popover placement flips", "Jonas Berg", "Nov 8", ST_PROGRESS, PRI_HIGH, { "Motion", NULL }, { CUI_TONE_VIOLET } },
    { "ENG-205", "Software renderer: SIMD path for text quads", "Lena Fischer", "Nov 12", ST_PROGRESS, PRI_MEDIUM, { "Perf", NULL }, { CUI_TONE_TEAL } },
    { "ENG-198", "Keyboard navigation for the color picker", "Priya Nair", "Nov 7", ST_REVIEW, PRI_HIGH, { "A11y", "Input" }, { CUI_TONE_INFO, CUI_TONE_NEUTRAL } },
    { "ENG-196", "Wayland backend through libdecor", "Tomas Rivera", "Nov 9", ST_REVIEW, PRI_MEDIUM, { "Platform", NULL }, { CUI_TONE_ROSE } },
    { "ENG-221", "Inline IME composition on macOS", "Aiko Tanaka", "Nov 14", ST_TODO, PRI_HIGH, { "Platform", "Input" }, { CUI_TONE_ROSE, CUI_TONE_NEUTRAL } },
    { "ENG-219", "Virtual table with sticky header", "Sam Whitfield", "Nov 18", ST_TODO, PRI_MEDIUM, { "Data", NULL }, { CUI_TONE_TEAL } },
    { "ENG-217", "Docs: theming cookbook", "Maya Okafor", "Nov 20", ST_TODO, PRI_LOW, { "Docs", NULL }, { CUI_TONE_NEUTRAL } },
    { "ENG-188", "Ship 1.0 release notes", "Jonas Berg", "Nov 1", ST_DONE, PRI_MEDIUM, { "Docs", NULL }, { CUI_TONE_NEUTRAL } },
    { "ENG-185", "Gliding highlight in segmented controls", "Priya Nair", "Oct 30", ST_DONE, PRI_LOW, { "Motion", NULL }, { CUI_TONE_VIOLET } },
};
#define ISSUE_COUNT ((int)(sizeof issues / sizeof issues[0]))

static const char *status_names[ST_COUNT] = { "Backlog", "Todo", "In progress", "In review", "Done" };
static const char *priority_names[] = { "No priority", "Low", "Medium", "High", "Urgent" };

typedef struct {
    int nav, team, tab, selected;
    bool collapsed[ST_COUNT];
    bool palette;
    char comment[256];
    bool subscribed;
    int panes;              /* 3, 2 or 1, from the window width */
    bool drawer, sheet;     /* the sidebar and the detail when they do not fit */
} app_state;

/* --- custom glyphs ------------------------------------------------------ */

typedef struct { int status; } status_glyph;
static void paint_status(cui_ctx *ui, cui_rect r, void *user) {
    status_glyph *g = user;
    cui_vec2 c = { r.x + r.w / 2, r.y + r.h / 2 };
    float rad = 6.5f;
    cui_color muted = cui_token_color(ui, CUI_TOKEN_TEXT_MUTED);
    cui_color warn = cui_token_color(ui, CUI_TOKEN_WARNING);
    cui_color ok = cui_token_color(ui, CUI_TOKEN_SUCCESS);
    cui_color accent = cui_token_color(ui, CUI_TOKEN_ACCENT);
    const float tau = 6.2831853f;
    switch (g->status) {
    case ST_BACKLOG:   /* dashed ring */
        for (int i = 0; i < 8; i++) cui_draw_arc(ui, c, rad, tau * i / 8 + 0.12f, tau * (i + 1) / 8 - 0.12f, 1.5f, muted);
        break;
    case ST_TODO:
        cui_draw_arc(ui, c, rad, 0, tau, 1.5f, cui_token_color(ui, CUI_TOKEN_TEXT_SECONDARY));
        break;
    case ST_PROGRESS:  /* ring with a half pie */
    case ST_REVIEW: {
        cui_color col = g->status == ST_PROGRESS ? warn : accent;
        float frac = g->status == ST_PROGRESS ? 0.5f : 0.75f;
        cui_draw_arc(ui, c, rad, 0, tau, 1.5f, col);
        cui_vec2 pts[34];
        int n = 0;
        pts[n++] = c;
        for (int i = 0; i <= 32; i++) {
            float a = -tau / 4 + tau * frac * i / 32;
            pts[n++] = (cui_vec2){ c.x + cosf(a) * (rad - 3), c.y + sinf(a) * (rad - 3) };
        }
        /* the pie is not convex for frac > 0.5: draw it as two halves */
        if (frac <= 0.5f) cui_draw_polygon(ui, pts, n, col);
        else {
            cui_draw_polygon(ui, pts, 18, col);
            cui_vec2 rest[18];
            rest[0] = c;
            for (int i = 0; i < 17; i++) rest[i + 1] = pts[17 + i];
            cui_draw_polygon(ui, rest, 18, col);
        }
        break;
    }
    case ST_DONE:
        cui_draw_circle(ui, c, rad + 0.75f, accent);
        cui_draw_icon(ui, CUI_ICON_CHECK, r, 10, cui_token_color(ui, CUI_TOKEN_ACCENT_FOREGROUND));
        (void)ok;
        break;
    }
}
static void status_icon(cui_ctx *ui, const char *id, int status) {
    status_glyph g = { status };
    cui_canvas(ui, id, CUI_PX(16), CUI_PX(16), paint_status, &g, sizeof g);
}

typedef struct { int priority; } priority_glyph;
static void paint_priority(cui_ctx *ui, cui_rect r, void *user) {
    priority_glyph *g = user;
    cui_color on = cui_token_color(ui, CUI_TOKEN_TEXT_SECONDARY);
    cui_color off = cui_alpha(on, 0.3f);
    if (g->priority == PRI_URGENT) {
        cui_rect b = { r.x + 1, r.y + 1, r.w - 2, r.h - 2 };
        cui_draw_rect(ui, b, CUI_CORNERS(3.5f), cui_token_color(ui, CUI_TOKEN_WARNING));
        cui_draw_rect(ui, (cui_rect){ b.x + b.w / 2 - 0.9f, b.y + 3, 1.8f, 5.5f }, CUI_CORNERS(0.9f), CUI_RGB(0x1a1a1a));
        cui_draw_rect(ui, (cui_rect){ b.x + b.w / 2 - 0.9f, b.y + b.h - 4.6f, 1.8f, 1.8f }, CUI_CORNERS(0.9f), CUI_RGB(0x1a1a1a));
        return;
    }
    if (g->priority == PRI_NONE) {
        for (int i = 0; i < 3; i++)
            cui_draw_rect(ui, (cui_rect){ r.x + 2 + i * 4.5f, r.y + r.h / 2 - 0.75f, 3, 1.5f }, CUI_CORNERS(0.75f), off);
        return;
    }
    for (int i = 0; i < 3; i++) {
        float h = 4 + i * 3.5f;
        cui_draw_rect(ui, (cui_rect){ r.x + 2 + i * 4.5f, r.y + r.h - 2 - h, 3, h }, CUI_CORNERS(1), i < g->priority ? on : off);
    }
}
static void priority_icon(cui_ctx *ui, const char *id, int priority) {
    priority_glyph g = { priority };
    cui_canvas(ui, id, CUI_PX(16), CUI_PX(16), paint_priority, &g, sizeof g);
}

/* A full-width hover/selection background behind a row of components. */
typedef struct { bool hovered, selected; float radius; } row_bg;
static void paint_row(cui_ctx *ui, cui_rect r, void *user) {
    row_bg *b = user;
    if (b->selected) cui_draw_rect(ui, r, CUI_CORNERS(b->radius), cui_token_color(ui, CUI_TOKEN_ACTIVE));
    else if (b->hovered) cui_draw_rect(ui, r, CUI_CORNERS(b->radius), cui_token_color(ui, CUI_TOKEN_HOVER));
}
static bool row_background(cui_ctx *ui, const char *id, bool selected, float radius) {
    cui_id rid = cui_make_id(ui, id);
    bool *hover = cui_state(ui, rid, sizeof(bool));
    row_bg b = { *hover, selected, radius };
    cui_canvas_result res = cui_canvas(ui, id, CUI_FILL, CUI_FILL, paint_row, &b, sizeof b);
    *hover = res.hovered;
    return res.clicked;
}

static void team_dot(cui_ctx *ui, cui_color c) {
    cui_box(ui, .width = CUI_PX(16), .height = CUI_PX(16), .radius = 5, .bg = c, .dir = CUI_STACK,
            .align = CUI_ALIGN_CENTER, .justify = CUI_JUSTIFY_CENTER) {}
}

/* --- panes -------------------------------------------------------------- */

static void sidebar(cui_ctx *ui, app_state *s, bool drawer) {
    cui_column(ui, .width = drawer ? CUI_FILL : CUI_PX(240), .height = CUI_FILL, .bg = drawer ? CUI_TRANSPARENT : CUI_COLOR_SURFACE_MUTED, .pad = drawer ? 0 : 12, .gap = 2) {
        cui_row(ui, .gap = 10, .pad_x = 6, .pad_top = 4, .pad_bottom = 12, .align = CUI_ALIGN_CENTER) {
            cui_box(ui, .width = CUI_PX(26), .height = CUI_PX(26), .radius = 8, .bg = CUI_COLOR_ACCENT, .dir = CUI_STACK,
                    .align = CUI_ALIGN_CENTER, .justify = CUI_JUSTIFY_CENTER) {
                cui_icon(ui, CUI_ICON_TARGET, 16, CUI_COLOR_ACCENT_FOREGROUND);
            }
            cui_text(ui, "Orbit", .font = CUI_FONT_MEDIUM, .size = 15, .width = CUI_FILL, .nowrap = true);
            cui_icon_button(ui, CUI_ICON_SQUARE_PEN, .size = CUI_SIZE_SM, .tooltip = "New issue");
        }
        static const char *main_items[] = { "Inbox", "My issues", "Views" };
        static const char *main_icons[] = { CUI_ICON_INBOX, CUI_ICON_CIRCLE_USER, CUI_ICON_LAYERS };
        static const char *main_badges[] = { "4", NULL, NULL };
        for (int i = 0; i < 3; i++)
            if (cui_nav_item(ui, main_items[i], .icon = main_icons[i], .badge = main_badges[i], .selected = s->nav == i, .group = "nav"))
                { s->nav = i; s->drawer = false; }
        cui_box(ui, .height = CUI_PX(12)) {}
        cui_nav_section(ui, "Workspace");
        static const char *ws_items[] = { "Projects", "Cycles", "Roadmap" };
        static const char *ws_icons[] = { CUI_ICON_BOX, CUI_ICON_REFRESH_CW, CUI_ICON_MAP_PIN };
        for (int i = 0; i < 3; i++)
            if (cui_nav_item(ui, ws_items[i], .icon = ws_icons[i], .selected = s->nav == 3 + i, .group = "nav")) { s->nav = 3 + i; s->drawer = false; }
        cui_box(ui, .height = CUI_PX(12)) {}
        cui_nav_section(ui, "Teams");
        static const char *teams[] = { "Engine", "Design", "Platform" };
        static const char *team_icons[] = { CUI_ICON_CPU, CUI_ICON_PALETTE, CUI_ICON_SERVER };
        for (int i = 0; i < 3; i++)
            if (cui_nav_item(ui, teams[i], .icon = team_icons[i], .selected = s->nav == 6 + i, .group = "nav")) s->nav = 6 + i;
        cui_spacer(ui);
        cui_card(ui, .pad = 14, .gap = 10, .muted = false) {
            cui_row(ui, .gap = 8, .align = CUI_ALIGN_CENTER) {
                cui_icon(ui, CUI_ICON_ZAP, 14, CUI_COLOR_ACCENT);
                cui_text(ui, "Cycle 24", .font = CUI_FONT_MEDIUM, .size = 13, .width = CUI_FILL);
                cui_text(ui, "6 days left", .size = 12, .color = CUI_COLOR_TEXT_MUTED);
            }
            cui_progress(ui, 0.64f, .height = 6);
            cui_text(ui, "32 of 50 issues done", .size = 12, .color = CUI_COLOR_TEXT_SECONDARY);
        }
        cui_box(ui, .height = CUI_PX(8)) {}
        cui_row(ui, .gap = 10, .pad_x = 6, .pad_y = 4, .align = CUI_ALIGN_CENTER) {
            cui_avatar(ui, "Maya Okafor", .size = 28, .status = "online");
            cui_column(ui, .gap = 0, .width = CUI_FILL) {
                cui_text(ui, "Maya Okafor", .font = CUI_FONT_MEDIUM, .size = 13, .nowrap = true);
                cui_text(ui, "maya@orbit.dev", .size = 12, .color = CUI_COLOR_TEXT_MUTED, .nowrap = true);
            }
            cui_icon_button(ui, CUI_ICON_SETTINGS, .size = CUI_SIZE_SM, .tooltip = "Settings");
        }
    }
}

static void issue_row(cui_ctx *ui, app_state *s, int i) {
    const issue *it = &issues[i];
    cui_push_id_int(ui, i);
    cui_box(ui, .dir = CUI_STACK, .width = CUI_FILL, .height = CUI_PX(44)) {
        if (row_background(ui, "bg", s->selected == i, 8)) { s->selected = i; s->sheet = s->panes < 3; }
        cui_row(ui, .width = CUI_FILL, .height = CUI_FILL, .pad_x = 12, .gap = 12, .align = CUI_ALIGN_CENTER) {
            priority_icon(ui, "prio", it->priority);
            cui_text(ui, it->key, .font = CUI_FONT_MONO, .size = 12, .color = CUI_COLOR_TEXT_MUTED, .width = CUI_PX(62), .nowrap = true);
            status_icon(ui, "status", it->status);
            cui_text(ui, it->title, .width = CUI_FILL, .nowrap = true);
            if (s->panes > 1) {   /* a phone keeps the title, the status and who has it */
                for (int l = 0; l < 2; l++)
                    if (it->labels[l]) cui_badge(ui, it->labels[l], .tone = it->label_tones[l]);
                cui_text(ui, it->date, .size = 12, .color = CUI_COLOR_TEXT_MUTED, .width = CUI_PX(46), .align = CUI_TEXT_RIGHT, .nowrap = true);
            }
            cui_avatar(ui, it->assignee, .size = 22);
        }
    }
    cui_pop_id(ui);
}

static void issue_list(cui_ctx *ui, app_state *s) {
    cui_column(ui, .width = CUI_FILL, .height = CUI_FILL) {
        bool phone = s->panes == 1;
        cui_row(ui, .width = CUI_FILL, .pad_x = phone ? 12 : 24, .pad_y = 14, .gap = phone ? 6 : 12, .align = CUI_ALIGN_CENTER) {
            if (phone && cui_icon_button(ui, CUI_ICON_MENU, .tooltip = "Menu")) s->drawer = true;
            static const char *crumbs[] = { "Engine", "Issues" };
            cui_breadcrumb(ui, crumbs, 2);
            cui_spacer(ui);
            if (phone) {
                cui_icon_button(ui, CUI_ICON_FILTER, .tooltip = "Filter");
                cui_icon_button(ui, CUI_ICON_PLUS, .variant = CUI_BUTTON_PRIMARY, .size = CUI_SIZE_SM, .tooltip = "New issue");
            } else {
                cui_button(ui, "Filter", .icon = CUI_ICON_FILTER, .size = CUI_SIZE_SM, .variant = CUI_BUTTON_GHOST);
                cui_button(ui, "Display", .icon = CUI_ICON_SLIDERS_HORIZONTAL, .size = CUI_SIZE_SM, .variant = CUI_BUTTON_GHOST);
                cui_button(ui, "New issue", .icon = CUI_ICON_PLUS, .size = CUI_SIZE_SM, .variant = CUI_BUTTON_PRIMARY);
            }
        }
        cui_row(ui, .width = CUI_FILL, .pad_x = phone ? 12 : 24, .pad_bottom = 12, .align = CUI_ALIGN_CENTER) {
            static const char *tabs[] = { "All issues", "Active", "Backlog" };
            cui_tabs(ui, "views", tabs, 3, &s->tab, .variant = CUI_TABS_PILL);
            cui_spacer(ui);
            static const char *people[] = { "Maya Okafor", "Jonas Berg", "Priya Nair", "Lena Fischer", "Aiko Tanaka" };
            cui_avatar_group(ui, people, 5, 4, 24);
        }
        cui_separator(ui);
        cui_scroll(ui, .id = "issues", .pad = phone ? 4 : 12, .gap = 2, .fade_edges = true) {
            static const int order[] = { ST_PROGRESS, ST_REVIEW, ST_TODO, ST_DONE };
            for (int g = 0; g < 4; g++) {
                int st = order[g], count = 0;
                for (int i = 0; i < ISSUE_COUNT; i++) count += issues[i].status == st;
                cui_push_id_int(ui, 100 + st);
                cui_row(ui, .width = CUI_FILL, .pad_x = 12, .pad_top = g ? 14 : 4, .pad_bottom = 6, .gap = 10, .align = CUI_ALIGN_CENTER) {
                    status_icon(ui, "group", st);
                    cui_text(ui, status_names[st], .font = CUI_FONT_MEDIUM, .size = 13, .nowrap = true);
                    char n[8];
                    snprintf(n, sizeof n, "%d", count);
                    cui_text(ui, n, .size = 13, .color = CUI_COLOR_TEXT_MUTED);
                    cui_spacer(ui);
                    cui_icon_button(ui, CUI_ICON_PLUS, .size = CUI_SIZE_SM, .tooltip = "Add issue");
                }
                cui_pop_id(ui);
                for (int i = 0; i < ISSUE_COUNT; i++)
                    if (issues[i].status == st) issue_row(ui, s, i);
            }
        }
    }
}

static void property(cui_ctx *ui, const char *name) {
    cui_text(ui, name, .size = 13, .color = CUI_COLOR_TEXT_SECONDARY, .width = CUI_PX(96), .nowrap = true);
}

static void detail(cui_ctx *ui, app_state *s, bool in_sheet) {
    const issue *it = &issues[s->selected];
    cui_column(ui, .width = in_sheet ? CUI_FILL : CUI_PX(380), .height = CUI_FILL, .bg = in_sheet ? CUI_TRANSPARENT : CUI_COLOR_SURFACE) {
        if (!in_sheet) cui_row(ui, .width = CUI_FILL, .pad_x = 20, .pad_y = 14, .gap = 6, .align = CUI_ALIGN_CENTER) {
            cui_text(ui, it->key, .font = CUI_FONT_MONO, .size = 12, .color = CUI_COLOR_TEXT_SECONDARY);
            cui_copy_button(ui, "copy", it->key);
            cui_spacer(ui);
            if (cui_icon_button(ui, CUI_ICON_BELL, .size = CUI_SIZE_SM, .tooltip = s->subscribed ? "Unsubscribe" : "Subscribe"))
                s->subscribed = !s->subscribed;
            cui_menu(ui, "More", .icon = CUI_ICON_ELLIPSIS, .icon_only = true, .variant = CUI_BUTTON_GHOST, .placement = CUI_PLACE_BOTTOM_END) {
                cui_menu_item(ui, "Copy link", .icon = CUI_ICON_LINK, .shortcut = "Ctrl L");
                cui_menu_item(ui, "Duplicate", .icon = CUI_ICON_COPY);
                cui_menu_separator(ui);
                cui_menu_item(ui, "Delete issue", .icon = CUI_ICON_TRASH_2, .danger = true);
            }
        }
        if (!in_sheet) cui_separator(ui);
        cui_scroll(ui, .id = "detail", .pad = in_sheet ? 0 : 20, .gap = 18, .fade_edges = true) {
            cui_text(ui, it->title, .font = CUI_FONT_DISPLAY, .size = 20, .line_height = 1.3f);
            cui_text(ui, "Typing quickly into a focused field sometimes loses the final keystroke when the frame "
                         "that receives it also changes focus. Repro: type and press Tab in the same frame.",
                     .color = CUI_COLOR_TEXT_SECONDARY);
            cui_column(ui, .gap = 12, .width = CUI_FILL) {
                cui_row(ui, .gap = 8, .align = CUI_ALIGN_CENTER) {
                    property(ui, "Status");
                    status_icon(ui, "d-status", it->status);
                    cui_text(ui, status_names[it->status], .size = 13);
                }
                cui_row(ui, .gap = 8, .align = CUI_ALIGN_CENTER) {
                    property(ui, "Priority");
                    priority_icon(ui, "d-prio", it->priority);
                    cui_text(ui, priority_names[it->priority], .size = 13);
                }
                cui_row(ui, .gap = 8, .align = CUI_ALIGN_CENTER) {
                    property(ui, "Assignee");
                    cui_avatar(ui, it->assignee, .size = 20);
                    cui_text(ui, it->assignee, .size = 13);
                }
                cui_row(ui, .gap = 6, .align = CUI_ALIGN_CENTER) {
                    property(ui, "Labels");
                    for (int l = 0; l < 2; l++)
                        if (it->labels[l]) cui_badge(ui, it->labels[l], .tone = it->label_tones[l]);
                }
                cui_row(ui, .gap = 8, .align = CUI_ALIGN_CENTER) {
                    property(ui, "Cycle");
                    cui_icon(ui, CUI_ICON_REFRESH_CW, 14, CUI_COLOR_TEXT_SECONDARY);
                    cui_text(ui, "Cycle 24", .size = 13);
                }
                cui_row(ui, .gap = 8, .align = CUI_ALIGN_CENTER) {
                    property(ui, "Due date");
                    cui_icon(ui, CUI_ICON_CALENDAR, 14, CUI_COLOR_TEXT_SECONDARY);
                    cui_text(ui, it->date, .size = 13);
                }
            }
            cui_separator(ui);
            cui_label(ui, "Activity");
            cui_column(ui, .gap = 0, .width = CUI_FILL) {
                cui_timeline_item(ui, "Maya moved this to In progress", .time = "2h ago", .icon = CUI_ICON_ARROW_RIGHT, .tone = CUI_TONE_WARNING);
                cui_timeline_item(ui, "Jonas linked a pull request", .time = "5h ago", .icon = CUI_ICON_GIT_BRANCH, .tone = CUI_TONE_ACCENT,
                                  .description = "Keep pending text across focus changes");
                cui_timeline_item(ui, "Priya set the priority to Urgent", .time = "Yesterday", .icon = CUI_ICON_CIRCLE_ALERT, .tone = CUI_TONE_WARNING);
                cui_timeline_item(ui, "Issue created", .time = "Nov 2", .icon = CUI_ICON_PLUS, .last = true);
            }
        }
        cui_separator(ui);
        cui_row(ui, .width = CUI_FILL, .pad = in_sheet ? 0 : 16, .pad_top = 16, .gap = 8, .align = CUI_ALIGN_CENTER) {
            cui_input(ui, "comment", s->comment, sizeof s->comment, .placeholder = "Leave a comment", .width = CUI_FILL);
            cui_icon_button(ui, CUI_ICON_SEND, .variant = CUI_BUTTON_PRIMARY, .tooltip = "Send");
        }
    }
}

static void frame(cui_ctx *ui, void *user) {
    app_state *s = user;
    if (cui_shortcut(ui, CUI_MOD_PRIMARY, CUI_KEY_K)) s->palette = true;
    float vw = cui_viewport_size(ui).x;
    s->panes = vw < 760 ? 1 : vw < 1200 ? 2 : 3;
    if (s->panes > 1) s->drawer = false;
    if (s->panes == 3) s->sheet = false;
    cui_row(ui, .width = CUI_FILL, .height = CUI_FILL, .align = CUI_ALIGN_STRETCH, .bg = CUI_COLOR_BACKGROUND) {
        if (s->panes > 1) { sidebar(ui, s, false); cui_separator(ui); }
        issue_list(ui, s);
        if (s->panes == 3) { cui_separator(ui); detail(ui, s, false); }
    }
    cui_sheet(ui, "Orbit", &s->drawer, .side = CUI_SIDE_LEFT, .size = 280) { sidebar(ui, s, true); }
    cui_sheet(ui, issues[s->selected].key, &s->sheet, .size = s->panes == 1 ? vw : 420) { detail(ui, s, true); }
    static const cui_command cmds[] = {
        { "Create issue", CUI_ICON_PLUS, "Issues", "C" },
        { "Assign to me", CUI_ICON_USER, "Issues", "I" },
        { "Change status", CUI_ICON_CIRCLE_DOT, "Issues", "S" },
        { "Go to inbox", CUI_ICON_INBOX, "Navigation", "G I" },
        { "Go to cycles", CUI_ICON_REFRESH_CW, "Navigation", "G C" },
    };
    int c = cui_command_palette(ui, &s->palette, cmds, 5);
    if (c >= 0) cui_toast(ui, cmds[c].title, .tone = CUI_TONE_SUCCESS);
}

int main(void) {
    static app_state s = { .nav = 6 };
    return cui_app_run(&(cui_app_desc){ .title = "Orbit", .width = 1440, .height = 900,
                                        .theme = CUI_THEME_DARK, .accent = CUI_ACCENT_NEUTRAL }, frame, &s);
}
