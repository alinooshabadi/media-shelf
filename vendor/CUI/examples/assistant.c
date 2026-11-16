/* Assistant: a chat app with reasoning, tool calls, a plan, sources and a
 * composer. Sending a message plays a short scripted answer. Dark by default.
 * The workspace panel closes below 1180 points and the chat list becomes a
 * drawer below 760. */
#include <cui/cui.h>
#include <cui/cui_app.h>

#include <stdio.h>
#include <string.h>

typedef struct {
    int chat;
    char draft[2048];
    char asked[2048];
    int model;
    int vote;
    double started;          /* when the current answer began, < 0 idle */
    bool asked_once;
    const char *files[4];
    int file_count;
    int removed;
    bool narrow;            /* a phone-sized window */
    bool drawer;            /* the chat list on a narrow window */
} app_state;

static const cui_model models[] = {
    { "Atlas", "Longer answers, reads files and code", "New", CUI_ICON_LAYERS },
    { "Atlas Fast", "Short answers in about a second", NULL, CUI_ICON_ZAP },
    { "Atlas Mini", "Runs on your own machine", NULL, CUI_ICON_BOX },
};

static const cui_source sources[] = {
    { "Immediate mode GUIs and why they keep coming back", "https://fieldnotes.dev/immediate-mode", "Retained and immediate mode toolkits and the trade offs between them." },
    { "Notes on subpixel text positioning", "https://glyphlab.io/notes/subpixel", "Shaping, hinting and subpixel positioning explained with examples." },
    { "Spring physics for interface motion", "https://motionbook.dev/springs", "Why spring physics feel more natural than cubic beziers for interface motion." },
};

static const char *answer =
    "Mostly because they do very little per frame. The interface is rebuilt from your data each frame, laid out "
    "in one pass and drawn in a handful of batches, so there is no tree of widget objects to keep in sync.\n\n"
    "Text is the expensive part, which is why glyphs are rasterized once into an atlas and reused. And motion "
    "feels quick when it is driven by springs that respond to input at once, instead of fixed length curves.";

static void sidebar(cui_ctx *ui, app_state *s, bool drawer) {
    cui_column(ui, .width = drawer ? CUI_FILL : CUI_PX(264), .height = CUI_FILL, .bg = drawer ? CUI_TRANSPARENT : CUI_COLOR_SURFACE_MUTED, .pad = drawer ? 0 : 12, .gap = 2) {
        cui_row(ui, .gap = 10, .pad_x = 6, .pad_top = 4, .pad_bottom = 10, .align = CUI_ALIGN_CENTER) {
            cui_box(ui, .width = CUI_PX(28), .height = CUI_PX(28), .radius = 9, .bg = CUI_COLOR_ACCENT, .dir = CUI_STACK,
                    .align = CUI_ALIGN_CENTER, .justify = CUI_JUSTIFY_CENTER) {
                cui_text(ui, "M", .font = CUI_FONT_DISPLAY, .size = 16, .color = CUI_COLOR_ACCENT_FOREGROUND, .line_height = 1.0f);
            }
            cui_text(ui, "Muse", .font = CUI_FONT_MEDIUM, .size = 15, .width = CUI_FILL, .nowrap = true);
            if (cui_icon_button(ui, CUI_ICON_SQUARE_PEN, .size = CUI_SIZE_SM, .tooltip = "New chat")) { s->asked_once = false; s->started = -1; s->draft[0] = 0; }
        }
        cui_nav_section(ui, "Today");
        static const char *today[] = { "Why immediate mode feels fast", "Quarterly report summary", "Spring presets for menus" };
        for (int i = 0; i < 3; i++)
            if (cui_nav_item(ui, today[i], .icon = CUI_ICON_MESSAGE_SQUARE, .selected = s->chat == i, .group = "chats")) { s->chat = i; s->drawer = false; }
        cui_box(ui, .height = CUI_PX(10)) {}
        cui_nav_section(ui, "Previous 7 days");
        static const char *week[] = { "Gantt chart layout ideas", "Kanban drag and drop", "Color palette for charts", "Release notes draft" };
        for (int i = 0; i < 4; i++)
            if (cui_nav_item(ui, week[i], .icon = CUI_ICON_MESSAGE_SQUARE, .selected = s->chat == 3 + i, .group = "chats")) { s->chat = 3 + i; s->drawer = false; }
        cui_spacer(ui);
        cui_card(ui, .pad = 14, .gap = 8) {
            cui_row(ui, .gap = 8, .align = CUI_ALIGN_CENTER) {
                cui_icon(ui, CUI_ICON_ZAP, 14, CUI_COLOR_ACCENT);
                cui_text(ui, "Usage this month", .font = CUI_FONT_MEDIUM, .size = 13, .width = CUI_FILL);
            }
            cui_progress(ui, 0.42f, .height = 6);
            cui_text(ui, "4,210 of 10,000 messages", .size = 12, .color = CUI_COLOR_TEXT_SECONDARY);
        }
        cui_box(ui, .height = CUI_PX(6)) {}
        cui_user_menu(ui, "Mara Quinn", .detail = "mara@muse.dev", .status = "online") {
            cui_menu_item(ui, "Settings", .icon = CUI_ICON_SETTINGS);
            cui_menu_item(ui, "Keyboard shortcuts", .icon = CUI_ICON_KEYBOARD);
            cui_menu_separator(ui);
            cui_menu_item(ui, "Log out", .icon = CUI_ICON_LOG_OUT);
        }
    }
}

static void answer_block(cui_ctx *ui, app_state *s, const char *key, float t, bool live) {
    cui_push_id(ui, key);
    bool thinking = live && t < 2.6f;
    cui_reasoning(ui, "think", .thinking = thinking, .seconds = live ? (thinking ? t : 2.6f) : 6) {
        cui_text(ui, "This is about perceived speed. Two parts: how little work a frame does, and how fast input reaches the screen.",
                 .size = 13.5f, .color = CUI_COLOR_TEXT_SECONDARY);
        cui_text(ui, "Find sources on retained against immediate mode, text rendering cost and interface motion.", .size = 13.5f, .color = CUI_COLOR_TEXT_SECONDARY);
    }
    if (!live || t > 1.2f) {
        cui_task_status st = live && t < 3.6f ? CUI_TASK_RUNNING : CUI_TASK_DONE;
        cui_tool_call(ui, "search", "web_search", .icon = CUI_ICON_GLOBE, .detail = "immediate mode gui performance", .status = st,
                      .seconds = st == CUI_TASK_DONE ? 1.4f : t - 1.2f,
                      .input = "{\"query\": \"immediate mode gui performance\", \"limit\": 5}",
                      .output = "{\"results\": 3, \"took_ms\": 1380}");
    }
    if (live && t > 2.6f && t < 4.2f) cui_agent_status(ui, "Reading 3 sources", .status = CUI_TASK_RUNNING, .seconds = t - 2.6f);
    if (!live || t > 3.6f) {
        if (live) cui_typewriter(ui, "answer", answer, .size = 15, .speed = 140, .caret = true);
        else cui_text(ui, answer, .size = 15);
        if (!live || t > 6.5f) {
            cui_row(ui, .gap = 6, .align = CUI_ALIGN_CENTER) {
                for (int k = 0; k < 3; k++) cui_citation(ui, k + 1, &sources[k]);
            }
            cui_source_list(ui, "sources", sources, 3);
            int act = cui_response_actions(ui, "actions", .copy_text = answer, .vote = &s->vote);
            if (act & CUI_RESPONSE_REGENERATE) s->started = cui_time(ui);
            if (act & CUI_RESPONSE_SHARE) cui_toast(ui, "Link copied", .tone = CUI_TONE_SUCCESS);
        }
    }
    cui_pop_id(ui);
}

static void conversation(cui_ctx *ui, app_state *s) {
    double now = cui_time(ui);
    float t = s->started >= 0 ? (float)(now - s->started) : 99;
    cui_column(ui, .width = CUI_FILL, .height = CUI_FILL) {
        cui_row(ui, .width = CUI_FILL, .pad_x = s->narrow ? 12 : 20, .pad_y = 12, .gap = s->narrow ? 6 : 10, .align = CUI_ALIGN_CENTER) {
            if (s->narrow && cui_icon_button(ui, CUI_ICON_MENU, .tooltip = "Chats")) s->drawer = true;
            cui_model_picker(ui, "model", models, 3, &s->model, .ghost = true, .placement = CUI_PLACE_BOTTOM_START);
            cui_spacer(ui);
            static const cui_viz_item parts[] = { { "Instructions", 3200 }, { "Files", 41800 }, { "Messages", 68400 } };
            cui_context_meter(ui, "ctx", 113400, 200000, .parts = parts, .part_count = 3);
            if (s->narrow) cui_icon_button(ui, CUI_ICON_SHARE_2, .tooltip = "Share");
            else cui_button(ui, "Share", .icon = CUI_ICON_SHARE_2, .variant = CUI_BUTTON_OUTLINE, .size = CUI_SIZE_SM);
        }
        cui_separator(ui);
        cui_scroll(ui, .id = "chat", .width = CUI_FILL, .height = CUI_FILL, .fade_edges = true) {
            cui_row(ui, .width = CUI_FILL, .justify = CUI_JUSTIFY_CENTER, .pad_x = s->narrow ? 12 : 24, .pad_y = s->narrow ? 16 : 24) {
                cui_column(ui, .width = CUI_FILL, .max_width = 760, .gap = 14) {
                    cui_chat_message(ui, "Why do immediate mode toolkits feel so fast? Cite a few sources.", .mine = true);
                    answer_block(ui, s, "first", t, false);
                    if (s->asked_once) {
                        cui_gap(ui, 12);
                        cui_chat_message(ui, s->asked, .mine = true);
                        answer_block(ui, s, "second", t, true);
                    }
                }
            }
        }
        cui_row(ui, .width = CUI_FILL, .justify = CUI_JUSTIFY_CENTER, .pad_x = s->narrow ? 12 : 24, .pad_bottom = s->narrow ? 12 : 20) {
            cui_column(ui, .width = CUI_FILL, .max_width = 760, .gap = 8) {
                bool busy = s->asked_once && t < 7.5f;
                int ev = cui_chat_composer(ui, "composer", s->draft, sizeof s->draft, .placeholder = "Ask a follow up", .busy = busy,
                                           .attachments = s->files, .attachment_count = s->file_count, .removed = &s->removed,
                                           .models = models, .model_count = 3, .model = &s->model);
                if (ev & CUI_COMPOSER_SEND) {
                    snprintf(s->asked, sizeof s->asked, "%s", s->draft);
                    s->draft[0] = 0;
                    s->asked_once = true;
                    s->started = cui_time(ui);
                }
                if (ev & CUI_COMPOSER_STOP) s->started = cui_time(ui) - 99;
                if ((ev & CUI_COMPOSER_REMOVE) && s->removed >= 0 && s->removed < s->file_count) {
                    for (int k = s->removed; k < s->file_count - 1; k++) s->files[k] = s->files[k + 1];
                    s->file_count--;
                }
                if (ev & CUI_COMPOSER_ATTACH) cui_toast(ui, "Drop files on the window to attach them");
                cui_text(ui, "Answers can be wrong. Each one links the sources it used.", .size = 12, .color = CUI_COLOR_TEXT_MUTED, .align = CUI_TEXT_CENTER, .width = CUI_FILL);
            }
        }
    }
}

static void side_panel(cui_ctx *ui, app_state *s) {
    double now = cui_time(ui);
    float t = s->asked_once && s->started >= 0 ? (float)(now - s->started) : 99;
    cui_column(ui, .width = CUI_PX(330), .height = CUI_FILL, .bg = CUI_COLOR_SURFACE, .pad = 20, .gap = 16) {
        cui_text(ui, "Workspace", .font = CUI_FONT_MEDIUM, .size = 15);
        cui_plan_step steps[] = {
            { "Search for sources", "3 results", t > 3.6f ? CUI_TASK_DONE : CUI_TASK_RUNNING },
            { "Read the sources", t > 4.2f ? "3 of 3" : "Reading", t > 4.2f ? CUI_TASK_DONE : t > 2.6f ? CUI_TASK_RUNNING : CUI_TASK_PENDING },
            { "Write the answer", NULL, t > 7.5f ? CUI_TASK_DONE : t > 3.6f ? CUI_TASK_RUNNING : CUI_TASK_PENDING },
            { "Check the citations", NULL, t > 7.5f ? CUI_TASK_DONE : CUI_TASK_PENDING },
        };
        cui_agent_plan(ui, "plan", steps, 4, .title = "Plan");
        cui_label(ui, "Files");
        cui_column(ui, .gap = 6, .width = CUI_FILL) {
            static const char *names[] = { "quarterly-report.pdf", "metrics.csv", "notes.md" };
            static const char *sizes[] = { "2.4 MB", "86 KB", "4 KB" };
            for (int k = 0; k < 3; k++) {
                cui_push_id_int(ui, k);
                cui_row(ui, .gap = 10, .align = CUI_ALIGN_CENTER, .width = CUI_FILL, .pad = 8, .radius = 12, .bg = CUI_COLOR_SURFACE_MUTED) {
                    cui_icon(ui, CUI_ICON_FILE_TEXT, 16, CUI_COLOR_TEXT_SECONDARY);
                    cui_text(ui, names[k], .size = 13, .width = CUI_FILL, .nowrap = true);
                    cui_text(ui, sizes[k], .size = 12, .color = CUI_COLOR_TEXT_MUTED);
                }
                cui_pop_id(ui);
            }
        }
        cui_label(ui, "Try asking");
        static const cui_prompt_suggestion sugg[] = {
            { CUI_ICON_CHART_BAR, "Compare with last quarter", "Revenue, churn and costs" },
            { CUI_ICON_LIST_CHECKS, "Turn this into a checklist", NULL },
        };
        int pick = cui_prompt_suggestions(ui, "sugg", sugg, 2, .columns = 1);
        if (pick >= 0) snprintf(s->draft, sizeof s->draft, "%s", sugg[pick].title);
    }
}

static void frame(cui_ctx *ui, void *user) {
    app_state *s = user;
    float vw = cui_viewport_size(ui).x;
    s->narrow = vw < 760;
    if (!s->narrow) s->drawer = false;
    cui_row(ui, .width = CUI_FILL, .height = CUI_FILL, .align = CUI_ALIGN_STRETCH, .bg = CUI_COLOR_BACKGROUND) {
        if (!s->narrow) { sidebar(ui, s, false); cui_separator(ui); }
        conversation(ui, s);
        if (vw >= 1180) { cui_separator(ui); side_panel(ui, s); }
    }
    cui_sheet(ui, "Muse", &s->drawer, .side = CUI_SIDE_LEFT, .size = 300) { sidebar(ui, s, true); }
}

int main(void) {
    static app_state s = { .started = -1, .files = { "quarterly-report.pdf", "metrics.csv" }, .file_count = 2, .removed = -1,
                           .draft = "Now compare these numbers with last quarter." };
    return cui_app_run(&(cui_app_desc){ .title = "Muse", .width = 1440, .height = 900,
                                        .theme = CUI_THEME_DARK, .accent = CUI_ACCENT_NEUTRAL }, frame, &s);
}
