/* Notes: a small but complete app. Split view, search, create, edit,
 * pin and delete notes, with keyboard shortcuts and a command palette.
 * On a phone the list and the editor take turns. */
#include <cui/cui.h>
#include <cui/cui_app.h>

#include <ctype.h>
#include <stdio.h>
#include <string.h>

#define MAX_NOTES 64

typedef struct {
    char title[96];
    char body[2048];
    bool pinned;
    int tag;
} note;

typedef struct {
    note notes[MAX_NOTES];
    int count;
    int selected;
    char query[64];
    float split;
    bool palette;
    bool confirm_delete;
    bool dark;
    int filter;
    bool one_pane;   /* a phone: the list or the open note */
    bool editing;
} notes_app;

static const char *tags[] = { "Personal", "Work", "Ideas" };

static bool matches(const note *n, const char *q) {
    if (!q[0]) return true;
    char hay[2200], needle[64];
    snprintf(hay, sizeof hay, "%s %s", n->title, n->body);
    snprintf(needle, sizeof needle, "%s", q);
    for (char *p = hay; *p; p++) *p = (char)tolower((unsigned char)*p);
    for (char *p = needle; *p; p++) *p = (char)tolower((unsigned char)*p);
    return strstr(hay, needle) != NULL;
}

static void add_note(notes_app *a, const char *title, const char *body, int tag) {
    if (a->count >= MAX_NOTES) return;
    note *n = &a->notes[a->count];
    memset(n, 0, sizeof *n);
    snprintf(n->title, sizeof n->title, "%s", title);
    snprintf(n->body, sizeof n->body, "%s", body);
    n->tag = tag;
    a->selected = a->count++;
    a->editing = true;
}

static void delete_selected(notes_app *a) {
    if (a->selected < 0 || a->selected >= a->count) return;
    memmove(&a->notes[a->selected], &a->notes[a->selected + 1], sizeof(note) * (size_t)(a->count - a->selected - 1));
    a->count--;
    if (a->selected >= a->count) a->selected = a->count - 1;
}

static void list_pane(cui_ctx *ui, notes_app *a) {
    cui_column(ui, .width = CUI_FILL, .height = CUI_FILL, .bg = CUI_COLOR_SURFACE_MUTED, .pad = 12, .gap = 10) {
        cui_row(ui, .gap = 8, .pad_x = 4, .pad_top = 4) {
            cui_text(ui, "Notes", .font = CUI_FONT_MEDIUM, .size = 16, .width = CUI_FILL, .nowrap = true);
            if (cui_icon_button(ui, CUI_ICON_SQUARE_PEN, .tooltip = "New note", .shortcut = "Ctrl N"))
                add_note(a, "Untitled", "", a->filter > 0 ? a->filter - 1 : 0);
        }
        cui_search(ui, "q", a->query, sizeof a->query, .placeholder = "Search notes", .size = CUI_SIZE_SM);
        const char *filters[] = { "All", "Personal", "Work", "Ideas" };
        cui_segmented(ui, "filter", filters, 4, &a->filter, .size = CUI_SIZE_SM, .full_width = true);
        cui_scroll(ui, .id = "list", .gap = 2, .fade_edges = true) {
            int shown = 0;
            for (int pass = 0; pass < 2; pass++) {
                for (int i = 0; i < a->count; i++) {
                    note *n = &a->notes[i];
                    if ((pass == 0) != n->pinned) continue;
                    if (a->filter > 0 && n->tag != a->filter - 1) continue;
                    if (!matches(n, a->query)) continue;
                    /* one line of the note: its lines joined with a dot, list markers dropped */
                    char preview[160];
                    int w = 0;
                    if (!n->body[0]) w = snprintf(preview, sizeof preview, "No content");
                    for (const char *p = n->body; *p && w < (int)sizeof preview - 8;) {
                        const char *e = strchr(p, '\n');
                        int len = e ? (int)(e - p) : (int)strlen(p);
                        const char *q = p;
                        while (len > 0 && (*q == '-' || *q == '*' || *q == ' ')) { q++; len--; }
                        if (len > 0) w += snprintf(preview + w, sizeof preview - (size_t)w, "%s%.*s", w ? "  \xc2\xb7  " : "", len, q);
                        if (!e) break;
                        p = e + 1;
                    }
                    cui_push_id_int(ui, i);
                    if (cui_list_item(ui, n->title[0] ? n->title : "Untitled", .subtitle = preview, .selected = a->selected == i,
                                      .icon = n->pinned ? CUI_ICON_BOOKMARK : CUI_ICON_FILE_TEXT)) {
                        a->selected = i;
                        a->editing = true;
                    }
                    cui_context_menu(ui, "note-menu") {
                        if (cui_menu_item(ui, n->pinned ? "Unpin" : "Pin to top", .icon = CUI_ICON_BOOKMARK)) n->pinned = !n->pinned;
                        if (cui_menu_item(ui, "Duplicate", .icon = CUI_ICON_COPY)) add_note(a, n->title, n->body, n->tag);
                        cui_menu_separator(ui);
                        if (cui_menu_item(ui, "Delete", .icon = CUI_ICON_TRASH_2, .danger = true)) { a->selected = i; a->confirm_delete = true; }
                    }
                    cui_pop_id(ui);
                    shown++;
                }
            }
            if (shown == 0) cui_empty_state(ui, "No notes found", .icon = CUI_ICON_SEARCH, .description = "Try another search or create a note.");
        }
    }
}

static void editor_pane(cui_ctx *ui, notes_app *a) {
    cui_column(ui, .width = CUI_FILL, .height = CUI_FILL, .pad_x = a->one_pane ? 16 : 40, .pad_y = a->one_pane ? 16 : 28, .gap = 16) {
        if (a->selected < 0 || a->selected >= a->count) {
            cui_spacer(ui);
            if (cui_empty_state(ui, "Nothing selected", .icon = CUI_ICON_NOTEBOOK_PEN, .description = "Pick a note on the left or start a new one.", .action = "New note"))
                add_note(a, "Untitled", "", 0);
            cui_spacer(ui);
            return;
        }
        note *n = &a->notes[a->selected];
        cui_row(ui, .gap = 8) {
            if (a->one_pane && cui_icon_button(ui, CUI_ICON_CHEVRON_LEFT, .tooltip = "All notes")) a->editing = false;
            cui_badge(ui, tags[n->tag], .tone = n->tag == 1 ? CUI_TONE_INFO : n->tag == 2 ? CUI_TONE_VIOLET : CUI_TONE_NEUTRAL);
            cui_text(ui, n->pinned ? "Pinned" : "", .size = 12, .color = CUI_COLOR_TEXT_MUTED, .nowrap = true, .width = CUI_FILL);
            if (cui_icon_button(ui, CUI_ICON_BOOKMARK, .tooltip = n->pinned ? "Unpin" : "Pin", .selected = n->pinned,
                                .color = n->pinned ? CUI_COLOR_ACCENT : CUI_TRANSPARENT)) n->pinned = !n->pinned;
            cui_menu(ui, "Note actions", .icon_only = true, .icon = CUI_ICON_ELLIPSIS, .placement = CUI_PLACE_BOTTOM_END) {
                for (int t = 0; t < 3; t++)
                    if (cui_menu_item(ui, tags[t], .icon = CUI_ICON_TAG, .checked = n->tag == t)) n->tag = t;
                cui_menu_separator(ui);
                if (cui_menu_item(ui, "Export as text", .icon = CUI_ICON_DOWNLOAD)) {
                    char path[1024], name[128];
                    snprintf(name, sizeof name, "%s.txt", n->title[0] ? n->title : "note");
                    if (cui_app_file_dialog(NULL, &(cui_file_dialog_opts){ .kind = CUI_FILE_SAVE, .title = "Export note",
                                                                         .filter_name = "Text", .filter = "txt", .default_name = name },
                                            path, sizeof path)) {
                        FILE *f = fopen(path, "wb");
                        if (f) { fprintf(f, "%s\n\n%s\n", n->title, n->body); fclose(f); cui_toast(ui, "Note exported", .description = path, .tone = CUI_TONE_SUCCESS); }
                        else cui_toast(ui, "Could not write the file", .tone = CUI_TONE_DANGER);
                    }
                }
                cui_menu_separator(ui);
                if (cui_menu_item(ui, "Delete note", .icon = CUI_ICON_TRASH_2, .danger = true, .shortcut = "Del")) a->confirm_delete = true;
            }
        }
        cui_push_id_int(ui, a->selected);
        cui_input(ui, "title", n->title, sizeof n->title, .placeholder = "Title", .size = CUI_SIZE_LG);
        cui_textarea(ui, "body", n->body, sizeof n->body, .placeholder = "Start writing", .height = -1);
        int words = 0;
        bool in_word = false;
        for (const char *p = n->body; *p; p++) {   /* a word has a letter or digit in it, so list dashes do not count */
            bool w = !isspace((unsigned char)*p);
            if (!w) { in_word = false; continue; }
            if (!in_word && isalnum((unsigned char)*p)) { words++; in_word = true; }
        }
        cui_textf(ui, "%d words  \xc2\xb7  %d characters", words, (int)strlen(n->body));
        cui_pop_id(ui);
    }
}

static void frame(cui_ctx *ui, void *user) {
    notes_app *a = user;
    if (cui_shortcut(ui, CUI_MOD_PRIMARY, CUI_KEY_N)) add_note(a, "Untitled", "", 0);
    if (cui_shortcut(ui, CUI_MOD_PRIMARY, CUI_KEY_K)) a->palette = true;
    if (!cui_wants_keyboard(ui) && cui_key_pressed(ui, CUI_KEY_DELETE) && a->count > 0) a->confirm_delete = true;
    a->one_pane = cui_viewport_size(ui).x < 640;
    cui_box(ui, .width = CUI_FILL, .height = CUI_FILL, .bg = CUI_COLOR_BACKGROUND) {
        if (!a->one_pane) {
            cui_split(ui, "split", &a->split, .min_first = 240, .min_second = 360) {
                list_pane(ui, a);
                cui_split_next(ui);
                editor_pane(ui, a);
            }
        } else if (a->editing && a->selected >= 0) {
            editor_pane(ui, a);
        } else {
            list_pane(ui, a);
        }
    }
    cui_dialog(ui, "Delete this note?", &a->confirm_delete, .description = "The note will be removed permanently.", .width = 380) {
        cui_dialog_footer(ui) {
            if (cui_button(ui, "Cancel")) a->confirm_delete = false;
            if (cui_button(ui, "Delete note", .variant = CUI_BUTTON_DANGER)) {
                delete_selected(a);
                a->confirm_delete = false;
                cui_toast(ui, "Note deleted");
            }
        }
    }
    static const cui_command cmds[] = {
        { "New note", CUI_ICON_SQUARE_PEN, "Notes", "Ctrl N" },
        { "Pin or unpin note", CUI_ICON_BOOKMARK, "Notes", NULL },
        { "Delete note", CUI_ICON_TRASH_2, "Notes", "Del" },
        { "Toggle dark mode", CUI_ICON_MOON, "View", NULL },
    };
    switch (cui_command_palette(ui, &a->palette, cmds, 4)) {
    case 0: add_note(a, "Untitled", "", 0); break;
    case 1: if (a->selected >= 0) a->notes[a->selected].pinned = !a->notes[a->selected].pinned; break;
    case 2: a->confirm_delete = true; break;
    case 3: a->dark = cui_get_theme_mode(ui) != CUI_THEME_DARK; cui_set_theme(ui, a->dark ? CUI_THEME_DARK : CUI_THEME_LIGHT); break;
    default: break;
    }
}

int main(void) {
    static notes_app a;
    a.split = 0.32f;
    add_note(&a, "Launch checklist", "Ship the 1.0 release.\n\n- Final pass on docs\n- Record the demo video\n- Write the announcement post\n- Celebrate", 1);
    add_note(&a, "Reading list", "The Design of Everyday Things\nA Pattern Language\nThe Art of Computer Programming", 0);
    add_note(&a, "App ideas", "A calm habit tracker with springy check marks. A tiny music player that lives in the menu bar.", 2);
    add_note(&a, "Meeting notes", "Discussed the roadmap for Q1. Focus on accessibility, keyboard support and docs.", 1);
    a.notes[0].pinned = true;
    a.selected = 0;
    a.editing = false;   /* a phone opens on the list */
    return cui_app_run(&(cui_app_desc){ .title = "Notes", .width = 1100, .height = 720, .accent = CUI_ACCENT_AMBER }, frame, &a);
}
