/* Mail: a three pane inbox with folders, search, a reading pane and a
 * compose dialog. Narrower windows drop to two panes with the folders in a
 * drawer, and a phone shows one pane at a time. */
#include <cui/cui.h>
#include <cui/cui_app.h>

#include <ctype.h>
#include <stdio.h>
#include <string.h>

typedef struct {
    const char *from, *email, *subject, *preview, *time, *body;
    int folder;            /* 0 inbox, 1 starred, 2 sent, 3 archive */
    bool unread, starred;
    int label;             /* -1 none, else tone */
} message;

static message messages[] = {
    { "Jonas Berg", "jonas@northlight.io", "Review of the onboarding flow", "I went through all six screens on a 13 inch laptop. Step three scrolls before the button is visible.", "9:41",
      "Hi Maya,\n\nI went through all six screens on a 13 inch laptop. Step three scrolls before the button is visible, and the skip link reads like an error.\n\nCould we look at it together on Thursday at 10?\n\nJonas", 0, true, true, CUI_TONE_INFO },
    { "Priya Nair", "priya@harbor.dev", "Invoice 2024-118", "The October invoice is attached. Payment terms are 30 days as usual.", "8:15",
      "Hi,\n\nThe October invoice is attached. Payment terms are 30 days as usual. Let me know if the purchase order number needs to change.\n\nPriya", 0, true, false, -1 },
    { "Tomas Rivera", "tomas@kiln.app", "Export is slow for large projects", "A 900 file project takes about four minutes to export on my machine.", "Yesterday",
      "A 900 file project takes about four minutes to export on my machine, most of it in the thumbnail step. I attached a trace.\n\nTomas", 0, false, true, CUI_TONE_WARNING },
    { "Lena Fischer", "lena@fischer.dev", "Re: Release notes draft", "Looks good. I tightened the second paragraph and fixed two links.", "Yesterday",
      "Looks good. I tightened the second paragraph and fixed two links that pointed at the old docs.\n\nLena", 0, false, false, CUI_TONE_SUCCESS },
    { "Aiko Tanaka", "aiko@tanaka.design", "Icons for the settings page", "Here are the twelve icons at 16 and 20 px, outline only.", "Mon",
      "Here are the twelve icons at 16 and 20 px, outline only, on the same 1.5 px stroke as the rest of the set.\n\nAiko", 0, false, false, CUI_TONE_VIOLET },
    { "Sam Whitfield", "sam@whitfield.co", "Office closed on Friday", "The building is closed for maintenance. Work from home if you can.", "Mon",
      "The building is closed for maintenance on Friday. Work from home if you can; the server room stays reachable over the VPN.\n\nSam", 3, false, true, -1 },
};
#define MESSAGE_COUNT ((int)(sizeof messages / sizeof messages[0]))

typedef struct {
    int folder, selected;
    char query[64];
    float split1, split2;
    bool compose;
    char to[96], subject[128], body[1024];
    bool dark;
    int panes;       /* 3, 2 or 1, from the window width */
    bool reading;    /* one pane: the message is open */
    bool drawer;     /* folders in a drawer below three panes */
} mail_app;

static bool match(const message *m, const char *q) {
    if (!q[0]) return true;
    char hay[1024], needle[64];
    snprintf(hay, sizeof hay, "%s %s %s", m->from, m->subject, m->preview);
    snprintf(needle, sizeof needle, "%s", q);
    for (char *p = hay; *p; p++) *p = (char)tolower((unsigned char)*p);
    for (char *p = needle; *p; p++) *p = (char)tolower((unsigned char)*p);
    return strstr(hay, needle) != NULL;
}

static bool in_folder(const message *m, int folder) {
    if (folder == 1) return m->starred;
    return m->folder == folder;
}

static void folders(cui_ctx *ui, mail_app *a, bool drawer) {
    static const char *names[] = { "Inbox", "Starred", "Sent", "Archive" };
    static const char *icons[] = { CUI_ICON_INBOX, CUI_ICON_STAR, CUI_ICON_SEND, CUI_ICON_ARCHIVE };
    cui_column(ui, .width = CUI_FILL, .height = CUI_FILL, .bg = drawer ? CUI_TRANSPARENT : CUI_COLOR_SURFACE_MUTED, .pad = drawer ? 0 : 12, .gap = 2) {
        cui_row(ui, .pad_x = 6, .pad_bottom = 12, .gap = 10) {
            cui_avatar(ui, "Maya Okafor", .size = 28);
            cui_column(ui, .gap = 0, .width = CUI_FILL) {
                cui_text(ui, "Maya Okafor", .font = CUI_FONT_MEDIUM, .nowrap = true);
                cui_text(ui, "maya@fieldwork.studio", .size = 12, .color = CUI_COLOR_TEXT_SECONDARY, .nowrap = true);
            }
        }
        if (cui_button(ui, "Compose", .variant = CUI_BUTTON_PRIMARY, .icon = CUI_ICON_SQUARE_PEN, .full_width = true)) { a->compose = true; a->drawer = false; }
        cui_gap(ui, 10);
        for (int f = 0; f < 4; f++) {
            int unread = 0;
            for (int i = 0; i < MESSAGE_COUNT; i++) if (in_folder(&messages[i], f) && messages[i].unread) unread++;
            char badge[8];
            snprintf(badge, sizeof badge, "%d", unread);
            if (cui_nav_item(ui, names[f], .icon = icons[f], .selected = a->folder == f, .badge = unread ? badge : NULL)) {
                a->folder = f;
                a->selected = -1;
                a->drawer = a->reading = false;
            }
        }
        cui_spacer(ui);
        cui_nav_section(ui, "Labels");
        static const char *labels[] = { "Work", "Personal", "Urgent" };
        static const int tones[] = { CUI_TONE_INFO, CUI_TONE_VIOLET, CUI_TONE_WARNING };
        for (int l = 0; l < 3; l++) {
            cui_row(ui, .gap = 10, .pad_x = 10, .pad_y = 6) {
                cui_badge(ui, labels[l], .tone = (cui_tone)tones[l]);
            }
        }
    }
}

/* list rows: a quiet fill when selected or hovered, a hairline between the others */
typedef struct { bool hovered, selected, last; } row_bg;
static void paint_row(cui_ctx *ui, cui_rect r, void *user) {
    row_bg *b = user;
    if (b->selected) cui_draw_rect(ui, r, CUI_CORNERS(10), cui_token_color(ui, CUI_TOKEN_ACTIVE));
    else if (b->hovered) cui_draw_rect(ui, r, CUI_CORNERS(10), cui_token_color(ui, CUI_TOKEN_HOVER));
    else if (!b->last) cui_draw_rect(ui, (cui_rect){ r.x + 26, r.y + r.h - 1, r.w - 38, 1 }, CUI_CORNERS(0), cui_token_color(ui, CUI_TOKEN_BORDER_SUBTLE));
}

static void message_list(cui_ctx *ui, mail_app *a) {
    static const char *names[] = { "Inbox", "Starred", "Sent", "Archive" };
    cui_column(ui, .width = CUI_FILL, .height = CUI_FILL, .gap = 10, .pad = 12) {
        cui_row(ui, .pad_x = 4, .pad_top = 4, .gap = 6, .width = CUI_FILL) {
            if (a->panes < 3 && cui_icon_button(ui, CUI_ICON_MENU, .tooltip = "Folders")) a->drawer = true;
            cui_box(ui, .width = CUI_FILL) { cui_heading(ui, names[a->folder], 4); }
            if (a->panes < 3 && cui_icon_button(ui, CUI_ICON_SQUARE_PEN, .tooltip = "Compose")) a->compose = true;
        }
        cui_search(ui, "search", a->query, sizeof a->query, .placeholder = "Search mail", .size = CUI_SIZE_SM);
        cui_scroll(ui, .id = "list", .gap = 0, .fade_edges = true) {
            int shown = 0;
            for (int i = 0; i < MESSAGE_COUNT; i++) {
                message *m = &messages[i];
                if (!in_folder(m, a->folder) || !match(m, a->query)) continue;
                shown++;
                cui_push_id_int(ui, i);
                int next = i + 1;
                while (next < MESSAGE_COUNT && (!in_folder(&messages[next], a->folder) || !match(&messages[next], a->query))) next++;
                cui_box(ui, .dir = CUI_STACK, .width = CUI_FILL) {
                    bool *hover = cui_state(ui, cui_make_id(ui, "bg"), sizeof(bool));
                    row_bg b = { *hover, a->selected == i, next >= MESSAGE_COUNT || a->selected == next };
                    cui_canvas_result res = cui_canvas(ui, "bg", CUI_FILL, CUI_FILL, paint_row, &b, sizeof b);
                    *hover = res.hovered;
                    if (res.clicked) { a->selected = i; m->unread = false; a->reading = true; }
                    /* every row keeps the dot's gutter, so names line up read or unread */
                    cui_row(ui, .width = CUI_FILL, .pad_y = 12, .pad_right = 12, .align = CUI_ALIGN_START) {
                        cui_box(ui, .width = CUI_PX(26), .pad_top = 6, .align = CUI_ALIGN_CENTER) {
                            if (m->unread) cui_box(ui, .width = CUI_PX(7), .height = CUI_PX(7), .radius = 4, .bg = CUI_COLOR_ACCENT) {}
                        }
                        cui_column(ui, .gap = 3, .width = CUI_FILL) {
                            cui_row(ui, .gap = 8, .width = CUI_FILL) {
                                cui_text(ui, m->from, .font = m->unread ? CUI_FONT_MEDIUM : CUI_FONT_BODY, .nowrap = true, .width = CUI_FILL);
                                cui_text(ui, m->time, .size = 12, .color = CUI_COLOR_TEXT_MUTED, .nowrap = true);
                            }
                            cui_text(ui, m->subject, .font = CUI_FONT_MEDIUM, .size = 12.5f, .nowrap = true);
                            cui_text(ui, m->preview, .size = 12, .color = CUI_COLOR_TEXT_SECONDARY, .nowrap = true);
                        }
                    }
                }
                cui_context_menu(ui, "row-menu") {
                    if (cui_menu_item(ui, m->unread ? "Mark as read" : "Mark as unread", .icon = CUI_ICON_MAIL)) m->unread = !m->unread;
                    if (cui_menu_item(ui, m->starred ? "Unstar" : "Star", .icon = CUI_ICON_STAR)) m->starred = !m->starred;
                    if (cui_menu_item(ui, "Archive", .icon = CUI_ICON_ARCHIVE)) m->folder = 3;
                }
                cui_pop_id(ui);
            }
            if (!shown) cui_empty_state(ui, "No messages", .icon = CUI_ICON_INBOX, .description = "You are all caught up.");
        }
    }
}

static void reading_pane(cui_ctx *ui, mail_app *a) {
    cui_column(ui, .width = CUI_FILL, .height = CUI_FILL) {
        if (a->selected < 0) {
            cui_column(ui, .width = CUI_FILL, .height = CUI_FILL, .justify = CUI_JUSTIFY_CENTER) {
                cui_empty_state(ui, "No message selected", .icon = CUI_ICON_MAIL, .description = "Choose a message to read it here.");
            }
            return;
        }
        message *m = &messages[a->selected];
        cui_row(ui, .gap = 4, .pad_x = a->panes == 1 ? 12 : 20, .pad_y = 10) {
            if (a->panes == 1 && cui_icon_button(ui, CUI_ICON_CHEVRON_LEFT, .tooltip = "Back to the list")) a->reading = false;
            if (cui_icon_button(ui, CUI_ICON_ARCHIVE, .tooltip = "Archive", .shortcut = "E")) { m->folder = 3; a->selected = -1; cui_toast(ui, "Archived", .action = "Undo"); }
            cui_icon_button(ui, CUI_ICON_TRASH_2, .tooltip = "Delete");
            if (cui_icon_button(ui, CUI_ICON_STAR, .tooltip = m->starred ? "Unstar" : "Star", .selected = m->starred,
                                .color = m->starred ? CUI_COLOR_WARNING : CUI_TRANSPARENT)) m->starred = !m->starred;
            cui_spacer(ui);
            if (cui_button(ui, "Reply", .icon = CUI_ICON_UNDO_2, .size = CUI_SIZE_SM)) {
                a->compose = true;
                snprintf(a->to, sizeof a->to, "%s", m->email);
                snprintf(a->subject, sizeof a->subject, "Re: %s", m->subject);
            }
        }
        cui_separator(ui);
        if (a->selected < 0) return;
        cui_scroll(ui, .id = "reader", .pad = a->panes == 1 ? 16 : 28, .gap = 18, .fade_edges = true) {
            cui_heading(ui, m->subject, 3);
            cui_row(ui, .gap = 12) {
                cui_avatar(ui, m->from, .size = 36);
                cui_column(ui, .gap = 1, .width = CUI_FILL) {
                    cui_text(ui, m->from, .font = CUI_FONT_MEDIUM, .nowrap = true);
                    cui_text(ui, m->email, .size = 12, .color = CUI_COLOR_TEXT_SECONDARY, .nowrap = true);
                }
                if (m->label >= 0) cui_badge(ui, m->label == CUI_TONE_INFO ? "Work" : m->label == CUI_TONE_VIOLET ? "Personal" : m->label == CUI_TONE_WARNING ? "Urgent" : "Merged",
                                             .tone = (cui_tone)m->label);
                cui_text(ui, m->time, .size = 12, .color = CUI_COLOR_TEXT_MUTED, .nowrap = true);
            }
            cui_text(ui, m->body, .line_height = 1.6f);
        }
    }
}

static void frame(cui_ctx *ui, void *user) {
    mail_app *a = user;
    if (!cui_wants_keyboard(ui) && cui_key_pressed(ui, CUI_KEY_C)) a->compose = true;
    float vw = cui_viewport_size(ui).x;
    a->panes = vw < 720 ? 1 : vw < 1100 ? 2 : 3;
    if (a->panes == 3) a->drawer = false;
    cui_box(ui, .width = CUI_FILL, .height = CUI_FILL, .bg = CUI_COLOR_BACKGROUND) {
        if (a->panes == 3) {
            cui_split(ui, "outer", &a->split1, .min_first = 200, .min_second = 600) {
                folders(ui, a, false);
                cui_split_next(ui);
                cui_split(ui, "inner", &a->split2, .min_first = 280, .min_second = 320) {
                    message_list(ui, a);
                    cui_split_next(ui);
                    reading_pane(ui, a);
                }
            }
        } else if (a->panes == 2) {
            cui_split(ui, "inner", &a->split2, .min_first = 260, .min_second = 320) {
                message_list(ui, a);
                cui_split_next(ui);
                reading_pane(ui, a);
            }
        } else if (a->reading && a->selected >= 0) {
            reading_pane(ui, a);
        } else {
            message_list(ui, a);
        }
    }
    cui_sheet(ui, "Mail", &a->drawer, .side = CUI_SIDE_LEFT, .size = 280) { folders(ui, a, true); }
    cui_dialog(ui, "New message", &a->compose, .width = 560) {
        cui_input(ui, "to", a->to, sizeof a->to, .label = "To", .icon = CUI_ICON_AT_SIGN, .autofocus = true);
        cui_input(ui, "subject", a->subject, sizeof a->subject, .label = "Subject");
        cui_textarea(ui, "body", a->body, sizeof a->body, .height = 180, .placeholder = "Write your message");
        cui_dialog_footer(ui) {
            if (cui_button(ui, "Discard", .variant = CUI_BUTTON_GHOST)) { a->compose = false; a->to[0] = a->subject[0] = a->body[0] = 0; }
            if (cui_button(ui, "Send", .variant = CUI_BUTTON_PRIMARY, .icon = CUI_ICON_SEND, .disabled = !a->to[0])) {
                a->compose = false;
                cui_toast(ui, "Message sent", .description = a->subject[0] ? a->subject : "No subject", .tone = CUI_TONE_SUCCESS);
                a->to[0] = a->subject[0] = a->body[0] = 0;
            }
        }
    }
}

int main(void) {
    static mail_app a = { .folder = 0, .selected = 0, .split1 = 0.2f, .split2 = 0.38f };
    messages[0].unread = false;
    return cui_app_run(&(cui_app_desc){ .title = "Mail", .width = 1280, .height = 800, .accent = CUI_ACCENT_BLUE }, frame, &a);
}
