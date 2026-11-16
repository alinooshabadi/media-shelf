/* Music player: generated cover art, a track list, a "made for you" grid and
 * a transport bar. Shows custom painting mixed with stock components. */
#include <cui/cui.h>
#include <cui/cui_app.h>

#include <math.h>
#include <stdio.h>
#include <string.h>

typedef struct { const char *title, *artist, *album; int seconds; int art; } track;

static const track tracks[] = {
    { "Low Tide", "Marlow Avenue", "Night Shift", 222, 0 },
    { "Static Bloom", "Velvet Circuit", "Soft Machines", 247, 1 },
    { "Low Orbit", "Kaito Mori", "Satellites", 198, 2 },
    { "Paper Lanterns", "The Quiet Hours", "Ember", 263, 3 },
    { "Glass Harbor", "Ilse Varga", "Tidal", 231, 4 },
    { "After Hours", "Hollis & Grey", "Night Shift", 214, 0 },
    { "Signal / Noise", "Pale Static", "Frequencies", 276, 5 },
};
#define TRACK_COUNT ((int)(sizeof tracks / sizeof tracks[0]))

/* Cover art: flat shapes in a few printed colors, like a screen printed
 * sleeve. Each index picks a palette and a composition. */
typedef struct { unsigned bg, a, b; } palette;
static const palette palettes[] = {
    { 0xc8553d, 0xf2e8cf, 0x2d3047 },   /* terracotta, cream, ink */
    { 0x2d3047, 0xe0a458, 0xf2e8cf },   /* ink, ochre, cream */
    { 0x5b8e7d, 0xf2e8cf, 0x2d3047 },   /* sage */
    { 0xe0a458, 0x2d3047, 0xc8553d },   /* ochre */
    { 0x1f3a5f, 0x8fb8de, 0xf2e8cf },   /* navy, sky */
    { 0xf2e8cf, 0xc8553d, 0x2d3047 },   /* cream */
};

typedef struct { int art; float radius; bool icon; } art_paint;

static void paint_art(cui_ctx *ui, cui_rect r, void *user) {
    art_paint *a = user;
    const palette *p = &palettes[a->art % 6];
    cui_color bg = CUI_RGB(p->bg), ca = CUI_RGB(p->a), cb = CUI_RGB(p->b);
    float s = r.w;
    cui_draw_rect(ui, r, CUI_CORNERS(a->radius), bg);
    cui_push_clip(ui, r);
    switch (a->art % 6) {
    case 0:   /* a low sun and a small moon */
        cui_draw_circle(ui, (cui_vec2){ r.x + s * 0.62f, r.y + s * 0.70f }, s * 0.36f, ca);
        cui_draw_circle(ui, (cui_vec2){ r.x + s * 0.24f, r.y + s * 0.26f }, s * 0.09f, cb);
        break;
    case 1:   /* three bands */
        for (int k = 0; k < 3; k++)
            cui_draw_rect(ui, (cui_rect){ r.x, r.y + s * (0.22f + k * 0.2f), s, s * 0.09f }, CUI_CORNERS(0), k == 1 ? cb : ca);
        break;
    case 2:   /* a quarter disc in the corner, a bar across */
        cui_draw_circle(ui, (cui_vec2){ r.x, r.y + s }, s * 0.62f, ca);
        cui_draw_rect(ui, (cui_rect){ r.x + s * 0.5f, r.y + s * 0.18f, s * 0.34f, s * 0.08f }, CUI_CORNERS(0), cb);
        break;
    case 3:   /* rings */
        for (int k = 3; k >= 1; k--)
            cui_draw_arc(ui, (cui_vec2){ r.x + s * 0.5f, r.y + s * 0.5f }, s * 0.12f * (float)k, 0, 6.2831853f, s * 0.035f, k == 2 ? cb : ca);
        break;
    case 4:   /* a grid of dots, one out of place */
        for (int y = 0; y < 4; y++)
            for (int x = 0; x < 4; x++)
                cui_draw_circle(ui, (cui_vec2){ r.x + s * (0.2f + x * 0.2f), r.y + s * (0.2f + y * 0.2f) }, s * 0.045f, (x == 2 && y == 1) ? cb : ca);
        break;
    default:  /* half a disc on a line */
        cui_draw_circle(ui, (cui_vec2){ r.x + s * 0.5f, r.y + s * 0.58f }, s * 0.3f, ca);
        cui_draw_rect(ui, (cui_rect){ r.x, r.y + s * 0.58f, s, s * 0.42f }, CUI_CORNERS(0), bg);
        cui_draw_rect(ui, (cui_rect){ r.x + s * 0.1f, r.y + s * 0.58f, s * 0.8f, s * 0.025f }, CUI_CORNERS(0), cb);
        break;
    }
    cui_pop_clip(ui);
    if (a->icon) cui_draw_icon(ui, CUI_ICON_MUSIC, r, s * 0.3f, cui_alpha(cb, 0.9f));
}
static void cover(cui_ctx *ui, const char *id, int art, float size, float radius) {
    art_paint a = { art, radius, false };
    cui_canvas(ui, id, CUI_PX(size), CUI_PX(size), paint_art, &a, sizeof a);
}

/* Equalizer bars for the playing row. */
typedef struct { float t; } eq_paint;
static void paint_eq(cui_ctx *ui, cui_rect r, void *user) {
    eq_paint *e = user;
    for (int i = 0; i < 3; i++) {
        float h = 4 + 8 * (0.5f + 0.5f * sinf(e->t * 6 + i * 1.9f));
        cui_draw_rect(ui, (cui_rect){ r.x + 2 + i * 4.5f, r.y + r.h - 2 - h, 3, h }, CUI_CORNERS(1), CUI_COLOR_ACCENT);
    }
}

typedef struct { bool hovered, selected; } row_bg;
static void paint_row(cui_ctx *ui, cui_rect r, void *user) {
    row_bg *b = user;
    if (b->selected) cui_draw_rect(ui, r, CUI_CORNERS(10), cui_token_color(ui, CUI_TOKEN_ACTIVE));
    else if (b->hovered) cui_draw_rect(ui, r, CUI_CORNERS(10), cui_token_color(ui, CUI_TOKEN_HOVER));
}
static bool row_background(cui_ctx *ui, bool selected) {
    bool *hover = cui_state(ui, cui_make_id(ui, "bg"), sizeof(bool));
    row_bg b = { *hover, selected };
    cui_canvas_result res = cui_canvas(ui, "bg", CUI_FILL, CUI_FILL, paint_row, &b, sizeof b);
    *hover = res.hovered;
    return res.clicked;
}

typedef struct {
    int nav, playing, playlist;
    bool paused, shuffle, liked[TRACK_COUNT];
    float position, volume;
    double time;
    char search[64];
    bool drawer;     /* the library drawer on a narrow window */
} app_state;

static bool compact(cui_ctx *ui) { return cui_viewport_size(ui).x < 820; }

static void fmt_time(char *out, size_t cap, int seconds) { snprintf(out, cap, "%d:%02d", seconds / 60, seconds % 60); }

static void library(cui_ctx *ui, app_state *s);
static void sidebar(cui_ctx *ui, app_state *s) {
    cui_column(ui, .width = CUI_PX(248), .height = CUI_FILL, .bg = CUI_COLOR_SURFACE_MUTED, .pad = 12, .gap = 2) {
        cui_row(ui, .gap = 10, .pad_x = 6, .pad_top = 4, .pad_bottom = 14, .align = CUI_ALIGN_CENTER) {
            cui_box(ui, .width = CUI_PX(26), .height = CUI_PX(26), .radius = 13, .bg = CUI_COLOR_ACCENT, .dir = CUI_STACK,
                    .align = CUI_ALIGN_CENTER, .justify = CUI_JUSTIFY_CENTER) {
                cui_icon(ui, CUI_ICON_AUDIO_LINES, 14, CUI_COLOR_ACCENT_FOREGROUND);
            }
            cui_text(ui, "Tempo", .font = CUI_FONT_MEDIUM, .size = 15);
        }
        library(ui, s);
    }
}

/* navigation and playlists: in the sidebar, or in a drawer on a narrow window */
static void library(cui_ctx *ui, app_state *s) {
    {
        static const char *items[] = { "Home", "Browse", "Radio" };
        static const char *icons[] = { CUI_ICON_HOUSE, CUI_ICON_COMPASS, CUI_ICON_RADIO };
        for (int i = 0; i < 3; i++)
            if (cui_nav_item(ui, items[i], .icon = icons[i], .selected = s->nav == i, .group = "nav")) { s->nav = i; s->drawer = false; }
        cui_box(ui, .height = CUI_PX(14)) {}
        cui_row(ui, .pad_right = 4, .align = CUI_ALIGN_CENTER) {
            cui_box(ui, .width = CUI_FILL) { cui_nav_section(ui, "Your library"); }
            cui_icon_button(ui, CUI_ICON_PLUS, .size = CUI_SIZE_SM, .tooltip = "New playlist");
        }
        static const char *lists[] = { "Late Night Coding", "Deep Focus", "Long Drive", "Morning Coffee", "Rainy Afternoons" };
        static const char *counts[] = { "48 songs", "112 songs", "64 songs", "37 songs", "29 songs" };
        cui_push_id(ui, "library");
        for (int i = 0; i < 5; i++) {
            cui_push_id_int(ui, i);
            cui_box(ui, .dir = CUI_STACK, .width = CUI_FILL, .height = CUI_PX(52)) {
                if (row_background(ui, s->playlist == i)) { s->playlist = i; s->drawer = false; }
                cui_row(ui, .width = CUI_FILL, .height = CUI_FILL, .pad_x = 8, .gap = 12, .align = CUI_ALIGN_CENTER) {
                    cover(ui, "art", i == 4 ? 5 : i, 38, 8);
                    cui_column(ui, .gap = 0, .width = CUI_FILL) {
                        cui_text(ui, lists[i], .font = CUI_FONT_MEDIUM, .size = 13, .nowrap = true);
                        cui_text(ui, counts[i], .size = 12, .color = CUI_COLOR_TEXT_MUTED, .nowrap = true);
                    }
                }
            }
            cui_pop_id(ui);
        }
        cui_pop_id(ui);
    }
}

static void hero(cui_ctx *ui, app_state *s) {
    bool narrow = compact(ui);
    cui_row(ui, .width = CUI_FILL, .gap = narrow ? 20 : 28, .align = CUI_ALIGN_END, .wrap = true) {
        art_paint a = { 0, 16, false };
        float art = narrow ? 148 : 196;
        cui_canvas(ui, "hero-art", CUI_PX(art), CUI_PX(art), paint_art, &a, sizeof a);
        cui_column(ui, .gap = 10, .width = CUI_FILL, .min_width = 260) {
            cui_text(ui, "Late Night Coding", .font = CUI_FONT_DISPLAY, .size = narrow ? 32 : 44, .line_height = 1.1f);
            cui_text(ui, "Warm synths and steady beats for the hours after everyone has logged off.",
                     .color = CUI_COLOR_TEXT_SECONDARY);
            cui_row(ui, .gap = 8, .align = CUI_ALIGN_CENTER) {
                cui_avatar(ui, "Mara Quinn", .size = 22);
                cui_text(ui, "Mara Quinn", .font = CUI_FONT_MEDIUM, .size = 13);
                cui_text(ui, "·  48 songs  ·  3 h 12 min", .size = 13, .color = CUI_COLOR_TEXT_MUTED);
            }
            cui_box(ui, .height = CUI_PX(4)) {}
            cui_row(ui, .gap = 8, .align = CUI_ALIGN_CENTER, .wrap = true) {
                if (cui_button(ui, s->paused ? "Play" : "Pause", .icon = s->paused ? CUI_ICON_PLAY : CUI_ICON_PAUSE,
                               .variant = CUI_BUTTON_PRIMARY, .size = CUI_SIZE_LG))
                    s->paused = !s->paused;
                if (cui_button(ui, "Shuffle", .icon = CUI_ICON_SHUFFLE, .size = CUI_SIZE_LG,
                               .variant = s->shuffle ? CUI_BUTTON_SECONDARY : CUI_BUTTON_OUTLINE))
                    s->shuffle = !s->shuffle;
                cui_icon_button(ui, CUI_ICON_HEART, .size = CUI_SIZE_LG, .tooltip = "Save to library");
                cui_icon_button(ui, CUI_ICON_DOWNLOAD, .size = CUI_SIZE_LG, .tooltip = "Download");
                cui_menu(ui, "More", .icon = CUI_ICON_ELLIPSIS, .icon_only = true, .variant = CUI_BUTTON_GHOST) {
                    cui_menu_item(ui, "Add to queue", .icon = CUI_ICON_LIST);
                    cui_menu_item(ui, "Share", .icon = CUI_ICON_SHARE_2);
                }
            }
        }
    }
}

static void track_list(cui_ctx *ui, app_state *s) {
    bool album = cui_viewport_size(ui).x >= 600;   /* a phone shows the title, the artist and the length */
    cui_column(ui, .width = CUI_FILL, .gap = 2) {
        cui_row(ui, .width = CUI_FILL, .pad_x = 14, .pad_bottom = 8, .gap = 14, .align = CUI_ALIGN_CENTER) {
            cui_text(ui, "#", .size = 12, .color = CUI_COLOR_TEXT_MUTED, .width = CUI_PX(18), .align = CUI_TEXT_CENTER);
            cui_text(ui, "Title", .size = 12, .color = CUI_COLOR_TEXT_MUTED, .width = CUI_FILL);
            if (album) cui_text(ui, "Album", .size = 12, .color = CUI_COLOR_TEXT_MUTED, .width = CUI_PX(180));
            cui_box(ui, .width = CUI_PX(28)) {}
            cui_box(ui, .width = CUI_PX(44), .align = CUI_ALIGN_END) { cui_icon(ui, CUI_ICON_CLOCK, 14, CUI_COLOR_TEXT_MUTED); }
        }
        cui_separator(ui);
        cui_box(ui, .height = CUI_PX(6)) {}
        cui_push_id(ui, "tracks");
        for (int i = 0; i < TRACK_COUNT; i++) {
            const track *t = &tracks[i];
            bool playing = s->playing == i;
            cui_push_id_int(ui, i);
            cui_box(ui, .dir = CUI_STACK, .width = CUI_FILL, .height = CUI_PX(56)) {
                if (row_background(ui, playing)) { s->playing = i; s->position = 0; s->paused = false; }
                cui_row(ui, .width = CUI_FILL, .height = CUI_FILL, .pad_x = 14, .gap = 14, .align = CUI_ALIGN_CENTER) {
                    if (playing && !s->paused) {
                        eq_paint e = { (float)s->time };
                        cui_canvas(ui, "eq", CUI_PX(18), CUI_PX(16), paint_eq, &e, sizeof e);
                    } else {
                        char n[8];
                        snprintf(n, sizeof n, "%d", i + 1);
                        cui_text(ui, n, .size = 13, .color = playing ? CUI_COLOR_ACCENT : CUI_COLOR_TEXT_MUTED, .width = CUI_PX(18),
                                 .align = CUI_TEXT_CENTER, .tabular = true);
                    }
                    cui_row(ui, .gap = 12, .width = CUI_FILL, .align = CUI_ALIGN_CENTER) {
                        cover(ui, "art", t->art, 40, 8);
                        cui_column(ui, .gap = 1, .width = CUI_FILL) {
                            cui_text(ui, t->title, .font = CUI_FONT_MEDIUM, .nowrap = true, .color = playing ? CUI_COLOR_ACCENT : CUI_COLOR_FOREGROUND);
                            cui_text(ui, t->artist, .size = 13, .color = CUI_COLOR_TEXT_SECONDARY, .nowrap = true);
                        }
                    }
                    if (album) cui_text(ui, t->album, .size = 13, .color = CUI_COLOR_TEXT_SECONDARY, .width = CUI_PX(180), .nowrap = true);
                    if (cui_icon_button(ui, CUI_ICON_HEART, .size = CUI_SIZE_SM,
                                        .color = s->liked[i] ? CUI_COLOR_ACCENT : CUI_COLOR_TEXT_MUTED,
                                        .tooltip = s->liked[i] ? "Remove from liked" : "Like"))
                        s->liked[i] = !s->liked[i];
                    char d[16];
                    fmt_time(d, sizeof d, t->seconds);
                    cui_text(ui, d, .size = 13, .color = CUI_COLOR_TEXT_SECONDARY, .width = CUI_PX(44), .align = CUI_TEXT_RIGHT, .tabular = true);
                }
            }
            cui_pop_id(ui);
        }
        cui_pop_id(ui);
    }
}

static void made_for_you(cui_ctx *ui) {
    static const char *names[] = { "Daily Mix 1", "Focus Flow", "Discover Weekly", "Release Radar" };
    static const char *desc[] = { "Hollis & Grey, Kaito Mori and more", "Instrumental, no lyrics", "New music picked for you", "Fresh from artists you follow" };
    cui_row(ui, .width = CUI_FILL, .align = CUI_ALIGN_CENTER) {
        cui_text(ui, "Made for you", .font = CUI_FONT_DISPLAY, .size = 20, .width = CUI_FILL);
        cui_button(ui, "Show all", .variant = CUI_BUTTON_GHOST, .size = CUI_SIZE_SM, .icon_right = CUI_ICON_ARROW_RIGHT);
    }
    cui_box(ui, .dir = CUI_GRID, .columns = 4, .min_column_width = 160, .gap = 16, .width = CUI_FILL) {
        for (int i = 0; i < 4; i++) {
            cui_push_id_int(ui, i);
            cui_card(ui, .pad = 12, .gap = 10, .hoverable = true) {
                art_paint a = { (i + 2) % 6, 10, false };
                cui_canvas(ui, "art", CUI_FILL, CUI_PX(150), paint_art, &a, sizeof a);
                cui_column(ui, .gap = 2, .width = CUI_FILL) {
                    cui_text(ui, names[i], .font = CUI_FONT_MEDIUM, .nowrap = true);
                    cui_text(ui, desc[i], .size = 12, .color = CUI_COLOR_TEXT_SECONDARY, .nowrap = true);
                }
            }
            cui_pop_id(ui);
        }
    }
}

static void player_bar(cui_ctx *ui, app_state *s) {
    const track *t = &tracks[s->playing];
    bool narrow = compact(ui);
    cui_row(ui, .width = CUI_FILL, .height = CUI_PX(narrow ? 72 : 84), .pad_x = narrow ? 12 : 20, .gap = narrow ? 8 : 24, .align = CUI_ALIGN_CENTER, .bg = CUI_COLOR_SURFACE) {
        cui_row(ui, .width = narrow ? CUI_FILL : CUI_PX(300), .gap = 12, .align = CUI_ALIGN_CENTER) {
            cover(ui, "now-art", t->art, 52, 10);
            cui_column(ui, .gap = 2, .width = CUI_FILL) {
                cui_text(ui, t->title, .font = CUI_FONT_MEDIUM, .nowrap = true);
                cui_text(ui, t->artist, .size = 12, .color = CUI_COLOR_TEXT_SECONDARY, .nowrap = true);
            }
            bool liked = s->liked[s->playing];
            if (cui_icon_button(ui, CUI_ICON_HEART, .size = CUI_SIZE_SM, .color = liked ? CUI_COLOR_ACCENT : CUI_COLOR_TEXT_MUTED,
                                .tooltip = liked ? "Remove from liked" : "Like"))
                s->liked[s->playing] = !liked;
        }
        if (narrow) {   /* a mini player: play and skip; the full controls need the width */
            if (cui_icon_button(ui, s->paused ? CUI_ICON_PLAY : CUI_ICON_PAUSE, .variant = CUI_BUTTON_PRIMARY, .size = CUI_SIZE_LG,
                                .tooltip = s->paused ? "Play" : "Pause")) s->paused = !s->paused;
            if (cui_icon_button(ui, CUI_ICON_SKIP_FORWARD, .tooltip = "Next")) { s->playing = (s->playing + 1) % TRACK_COUNT; s->position = 0; }
            return;
        }
        cui_column(ui, .width = CUI_FILL, .gap = 6, .align = CUI_ALIGN_CENTER) {
            cui_row(ui, .gap = 10, .align = CUI_ALIGN_CENTER) {
                if (cui_icon_button(ui, CUI_ICON_SHUFFLE, .size = CUI_SIZE_SM, .selected = s->shuffle,
                                    .color = s->shuffle ? CUI_COLOR_ACCENT : CUI_TRANSPARENT, .tooltip = "Shuffle")) s->shuffle = !s->shuffle;
                if (cui_icon_button(ui, CUI_ICON_SKIP_BACK, .tooltip = "Previous")) { s->playing = (s->playing + TRACK_COUNT - 1) % TRACK_COUNT; s->position = 0; }
                if (cui_icon_button(ui, s->paused ? CUI_ICON_PLAY : CUI_ICON_PAUSE, .variant = CUI_BUTTON_PRIMARY, .size = CUI_SIZE_LG,
                                    .tooltip = s->paused ? "Play" : "Pause")) s->paused = !s->paused;
                if (cui_icon_button(ui, CUI_ICON_SKIP_FORWARD, .tooltip = "Next")) { s->playing = (s->playing + 1) % TRACK_COUNT; s->position = 0; }
                cui_icon_button(ui, CUI_ICON_REPEAT, .size = CUI_SIZE_SM, .tooltip = "Repeat");
            }
            cui_row(ui, .width = CUI_FILL, .gap = 12, .align = CUI_ALIGN_CENTER) {
                char a[16], b[16];
                fmt_time(a, sizeof a, (int)s->position);
                fmt_time(b, sizeof b, t->seconds);
                cui_text(ui, a, .size = 12, .color = CUI_COLOR_TEXT_MUTED, .tabular = true, .width = CUI_PX(36), .align = CUI_TEXT_RIGHT);
                cui_slider(ui, "##position", &s->position, 0, (float)t->seconds, .width = CUI_FILL);
                cui_text(ui, b, .size = 12, .color = CUI_COLOR_TEXT_MUTED, .tabular = true, .width = CUI_PX(36));
            }
        }
        cui_row(ui, .width = CUI_PX(300), .gap = 8, .align = CUI_ALIGN_CENTER, .justify = CUI_JUSTIFY_END) {
            cui_icon_button(ui, CUI_ICON_MIC, .size = CUI_SIZE_SM, .tooltip = "Lyrics");
            cui_icon_button(ui, CUI_ICON_LIST, .size = CUI_SIZE_SM, .tooltip = "Queue");
            cui_icon(ui, s->volume > 0 ? CUI_ICON_VOLUME_2 : CUI_ICON_VOLUME_X, 16, CUI_COLOR_TEXT_SECONDARY);
            cui_slider(ui, "##volume", &s->volume, 0, 100, .width = CUI_PX(110));
        }
    }
}

static void frame(cui_ctx *ui, void *user) {
    app_state *s = user;
    double now = cui_time(ui);
    float dt = s->time > 0 ? (float)(now - s->time) : 0;
    if (dt > 0.25f) dt = 0.25f;
    s->time = now;
    if (!s->paused) {
        s->position += dt;
        if (s->position >= tracks[s->playing].seconds) { s->playing = (s->playing + 1) % TRACK_COUNT; s->position = 0; }
        cui_request_redraw(ui);   /* keep the clock and the equalizer moving */
    }
    if (!cui_wants_keyboard(ui) && cui_key_pressed(ui, CUI_KEY_SPACE)) s->paused = !s->paused;
    cui_column(ui, .width = CUI_FILL, .height = CUI_FILL, .bg = CUI_COLOR_BACKGROUND) {
        bool narrow = compact(ui);
        if (!narrow) s->drawer = false;
        cui_row(ui, .width = CUI_FILL, .height = CUI_FILL, .align = CUI_ALIGN_STRETCH) {
            if (!narrow) { sidebar(ui, s); cui_separator(ui); }
            cui_column(ui, .width = CUI_FILL) {
                cui_row(ui, .width = CUI_FILL, .pad_x = narrow ? 12 : 32, .pad_y = 14, .gap = 8, .align = CUI_ALIGN_CENTER) {
                    if (narrow) {
                        if (cui_icon_button(ui, CUI_ICON_MENU, .tooltip = "Library")) s->drawer = true;
                    } else {
                        cui_icon_button(ui, CUI_ICON_CHEVRON_LEFT, .tooltip = "Back");
                        cui_icon_button(ui, CUI_ICON_CHEVRON_RIGHT, .tooltip = "Forward");
                    }
                    cui_box(ui, .width = CUI_GROW(4), .max_width = 320) {
                        cui_search(ui, "search", s->search, sizeof s->search, .placeholder = "Songs, artists, podcasts");
                    }
                    cui_spacer(ui);
                    cui_icon_button(ui, CUI_ICON_BELL, .tooltip = "What's new");
                    cui_avatar(ui, "Mara Quinn", .size = 30);
                }
                cui_scroll(ui, .id = "main", .pad = narrow ? 16 : 32, .gap = narrow ? 20 : 28, .fade_edges = true) {
                    hero(ui, s);
                    track_list(ui, s);
                    made_for_you(ui);
                }
            }
        }
        cui_separator(ui);
        player_bar(ui, s);
    }
    cui_sheet(ui, "Tempo", &s->drawer, .side = CUI_SIDE_LEFT, .size = 300) { library(ui, s); }
}

int main(void) {
    static app_state s = { .position = 74, .volume = 70, .liked = { true, false, true } };
    return cui_app_run(&(cui_app_desc){ .title = "Tempo", .width = 1440, .height = 1180,
                                        .theme = CUI_THEME_DARK, .accent = CUI_ACCENT_CORAL }, frame, &s);
}
