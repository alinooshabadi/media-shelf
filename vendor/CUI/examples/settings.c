/* Account settings: section navigation, forms, preferences and a danger zone.
 * On a narrow window the sections become tabs and each setting's label sits
 * above its control. */
#include <cui/cui.h>
#include <cui/cui_app.h>

#include <stdio.h>
#include <string.h>

typedef struct {
    int section;
    char first[64], last[64], email[96], username[48], bio[400], website[96];
    int timezone, language;
    bool email_comments, email_mentions, email_digest, push_all, push_mentions, desktop;
    int theme, density;
    int accent;
    float font_scale;
    bool reduced_motion;
    bool confirm_delete;
    char confirm_text[64];
    bool dirty;
} settings;

static void field_row(cui_ctx *ui, const char *title, const char *desc) {
    cui_column(ui, .width = CUI_FILL, .min_width = 200, .max_width = 240, .gap = 3) {
        cui_label(ui, title);
        if (desc) cui_text(ui, desc, .size = 12, .color = CUI_COLOR_TEXT_SECONDARY);
    }
}

static void profile(cui_ctx *ui, settings *s) {
    cui_card(ui, .gap = 24) {
        cui_row(ui, .gap = 16) {
            cui_avatar(ui, "Maya Okafor", .size = 64);
            cui_column(ui, .gap = 8, .width = CUI_FILL) {
                cui_label(ui, "Profile photo");
                cui_row(ui, .gap = 8) {
                    cui_button(ui, "Upload", .icon = CUI_ICON_UPLOAD, .size = CUI_SIZE_SM);
                    cui_button(ui, "Remove", .variant = CUI_BUTTON_GHOST, .size = CUI_SIZE_SM);
                }
            }
        }
        cui_separator(ui);
        cui_box(ui, .dir = CUI_GRID, .columns = 2, .min_column_width = 220, .gap = 16) {
            if (cui_input(ui, "first", s->first, sizeof s->first, .label = "First name") & CUI_CHANGED) s->dirty = true;
            if (cui_input(ui, "last", s->last, sizeof s->last, .label = "Last name") & CUI_CHANGED) s->dirty = true;
            if (cui_input(ui, "email", s->email, sizeof s->email, .label = "Email", .icon = CUI_ICON_MAIL,
                          .hint = "Used for sign in and notifications.") & CUI_CHANGED) s->dirty = true;
            if (cui_input(ui, "user", s->username, sizeof s->username, .label = "Username", .icon = CUI_ICON_AT_SIGN,
                          .error = strchr(s->username, ' ') ? "Usernames cannot contain spaces." : NULL) & CUI_CHANGED) s->dirty = true;
        }
        if (cui_input(ui, "web", s->website, sizeof s->website, .label = "Website", .icon = CUI_ICON_GLOBE, .placeholder = "https://") & CUI_CHANGED) s->dirty = true;
        if (cui_textarea(ui, "bio", s->bio, sizeof s->bio, .label = "Bio", .height = 100, .hint = "A few words about you. Shown on your public profile.") & CUI_CHANGED)
            s->dirty = true;
        cui_box(ui, .dir = CUI_GRID, .columns = 2, .min_column_width = 220, .gap = 16) {
            static const char *zones[] = { "Pacific Time (UTC-8)", "Eastern Time (UTC-5)", "London (UTC+0)", "Berlin (UTC+1)", "Tehran (UTC+3:30)", "Tokyo (UTC+9)" };
            if (cui_select(ui, "tz", zones, 6, &s->timezone, .label = "Time zone", .searchable = true)) s->dirty = true;
            static const char *langs[] = { "English", "Deutsch", "Español", "Français", "Italiano", "Português" };
            if (cui_select(ui, "lang", langs, 6, &s->language, .label = "Language")) s->dirty = true;
        }
    }
}

static void notifications(cui_ctx *ui, settings *s) {
    cui_card(ui, .gap = 20) {
        cui_row(ui, .align = CUI_ALIGN_START, .gap = 24, .wrap = true, .width = CUI_FILL) {
            field_row(ui, "Email", "What we send to your inbox.");
            cui_column(ui, .gap = 14, .width = CUI_FILL) {
                s->dirty |= cui_checkbox(ui, "Comments", &s->email_comments, .description = "When someone replies to your comment.");
                s->dirty |= cui_checkbox(ui, "Mentions", &s->email_mentions, .description = "When someone mentions you.");
                s->dirty |= cui_checkbox(ui, "Weekly digest", &s->email_digest, .description = "A summary of activity every Monday.");
            }
        }
        cui_separator(ui);
        cui_row(ui, .align = CUI_ALIGN_START, .gap = 24, .wrap = true, .width = CUI_FILL) {
            field_row(ui, "Push", "Notifications on your devices.");
            cui_column(ui, .gap = 14, .width = CUI_FILL) {
                s->dirty |= cui_switch(ui, "Everything", &s->push_all);
                s->dirty |= cui_switch(ui, "Mentions only", &s->push_mentions);
                s->dirty |= cui_switch(ui, "Desktop notifications", &s->desktop, .description = "Show a banner even when the app is in the background.");
            }
        }
    }
}

static void appearance(cui_ctx *ui, settings *s) {
    cui_card(ui, .gap = 20) {
        cui_row(ui, .gap = 24, .wrap = true, .width = CUI_FILL) {
            field_row(ui, "Theme", "Choose how the app looks.");
            const char *themes[] = { "Light", "Dark" };
            const char *icons[] = { CUI_ICON_SUN, CUI_ICON_MOON };
            if (cui_segmented(ui, "theme", themes, 2, &s->theme, .icons = icons)) cui_set_theme(ui, (cui_theme_mode)s->theme);
        }
        cui_separator(ui);
        cui_row(ui, .gap = 24, .wrap = true, .width = CUI_FILL) {
            field_row(ui, "Accent", "Used for selection and focus.");
            static const char *accents[] = { "Neutral", "Violet", "Blue", "Green", "Amber", "Orange", "Coral", "Rose" };
            if (cui_select(ui, "accent", accents, 8, &s->accent, .width = CUI_PX(200))) cui_set_accent(ui, (cui_accent)s->accent);
        }
        cui_separator(ui);
        cui_row(ui, .gap = 24, .wrap = true, .width = CUI_FILL) {
            field_row(ui, "Density", NULL);
            const char *d[] = { "Comfortable", "Compact" };
            cui_segmented(ui, "density", d, 2, &s->density, .size = CUI_SIZE_SM);
        }
        cui_separator(ui);
        cui_row(ui, .gap = 24, .wrap = true, .width = CUI_FILL) {
            field_row(ui, "Text size", "Scales all text in the app.");
            if (cui_slider(ui, "##fontscale", &s->font_scale, 0.85f, 1.3f, .step = 0.05f, .show_value = true, .format = "%.2fx"))
                cui_theme_get(ui)->font_scale = s->font_scale;
        }
        cui_separator(ui);
        cui_row(ui, .gap = 24, .wrap = true, .width = CUI_FILL) {
            field_row(ui, "Reduce motion", "Keep state changes, drop travel and bounce.");
            if (cui_switch(ui, "##motion", &s->reduced_motion)) cui_set_reduced_motion(ui, s->reduced_motion);
        }
    }
}

static void account(cui_ctx *ui, settings *s) {
    cui_card(ui, .gap = 16) {
        cui_label(ui, "Sessions");
        cui_list_item(ui, "MacBook Pro, Berlin", .icon = CUI_ICON_MONITOR, .subtitle = "This device", .trailing = "Now");
        cui_list_item(ui, "Windows desktop, Tehran", .icon = CUI_ICON_MONITOR, .subtitle = "Chrome", .trailing = "2h ago");
        cui_list_item(ui, "Linux laptop, Tokyo", .icon = CUI_ICON_MONITOR, .subtitle = "Firefox", .trailing = "Yesterday");
    }
    cui_card(ui, .gap = 14) {
        cui_row(ui, .gap = 16, .wrap = true, .width = CUI_FILL) {
            cui_column(ui, .gap = 4, .width = CUI_FILL, .min_width = 220) {
                cui_text(ui, "Delete account", .font = CUI_FONT_MEDIUM, .color = CUI_COLOR_DANGER);
                cui_secondary(ui, "Permanently remove your account and all of its data. This cannot be undone.");
            }
            if (cui_button(ui, "Delete account", .variant = CUI_BUTTON_DANGER)) { s->confirm_delete = true; s->confirm_text[0] = 0; }
        }
    }
    cui_dialog(ui, "Delete account", &s->confirm_delete, .description = "Type DELETE to confirm. Your projects, comments and files will be removed.") {
        cui_input(ui, "confirm", s->confirm_text, sizeof s->confirm_text, .placeholder = "DELETE", .autofocus = true);
        cui_dialog_footer(ui) {
            if (cui_button(ui, "Cancel")) s->confirm_delete = false;
            if (cui_button(ui, "Delete account", .variant = CUI_BUTTON_DANGER, .disabled = strcmp(s->confirm_text, "DELETE") != 0)) {
                s->confirm_delete = false;
                cui_toast(ui, "Account scheduled for deletion", .description = "You have 14 days to change your mind.", .tone = CUI_TONE_DANGER);
            }
        }
    }
}

static void frame(cui_ctx *ui, void *user) {
    settings *s = user;
    static const char *sections[] = { "Profile", "Notifications", "Appearance", "Account" };
    static const char *icons[] = { CUI_ICON_USER, CUI_ICON_BELL, CUI_ICON_PALETTE, CUI_ICON_SHIELD };
    static const char *descs[] = { "How others see you.", "Choose what you hear about.", "Make it yours.", "Security and data." };
    bool narrow = cui_viewport_size(ui).x < 760;
    cui_scroll(ui, .id = "page", .width = CUI_FILL, .fade_edges = true) {
        cui_row(ui, .justify = CUI_JUSTIFY_CENTER, .align = CUI_ALIGN_START) {
            cui_column(ui, .width = CUI_FILL, .max_width = 1040, .pad_x = narrow ? 16 : 40, .pad_top = narrow ? 20 : 40, .pad_bottom = 80, .gap = narrow ? 20 : 28) {
                cui_column(ui, .gap = 6) {
                    const char *crumbs[] = { "Workspace", "Settings", sections[s->section] };
                    cui_breadcrumb(ui, crumbs, 3);
                    cui_heading(ui, "Settings", 1);
                }
                if (narrow) cui_tabs(ui, "sections", sections, 4, &s->section, .variant = CUI_TABS_PILL, .icons = icons);
                cui_row(ui, .gap = 40, .align = CUI_ALIGN_START) {
                    if (!narrow) cui_column(ui, .width = CUI_PX(220), .gap = 2) {
                        for (int i = 0; i < 4; i++)
                            if (cui_nav_item(ui, sections[i], .icon = icons[i], .selected = s->section == i, .group = "sections")) s->section = i;
                    }
                    cui_column(ui, .width = CUI_FILL, .gap = 20) {
                        cui_column(ui, .gap = 4) {
                            cui_heading(ui, sections[s->section], 3);
                            cui_secondary(ui, descs[s->section]);
                        }
                        switch (s->section) {
                        case 0: profile(ui, s); break;
                        case 1: notifications(ui, s); break;
                        case 2: appearance(ui, s); break;
                        default: account(ui, s); break;
                        }
                        cui_row(ui, .gap = 8, .justify = CUI_JUSTIFY_END) {
                            if (cui_button(ui, "Discard", .variant = CUI_BUTTON_GHOST, .disabled = !s->dirty)) s->dirty = false;
                            if (cui_button(ui, "Save changes", .variant = CUI_BUTTON_PRIMARY, .disabled = !s->dirty, .icon = CUI_ICON_CHECK)) {
                                s->dirty = false;
                                cui_toast(ui, "Settings saved", .tone = CUI_TONE_SUCCESS);
                            }
                        }
                    }
                }
            }
        }
    }
}

int main(void) {
    static settings s;
    snprintf(s.first, sizeof s.first, "Maya");
    snprintf(s.last, sizeof s.last, "Okafor");
    snprintf(s.email, sizeof s.email, "maya@fieldwork.studio");
    snprintf(s.username, sizeof s.username, "maya.okafor");
    snprintf(s.website, sizeof s.website, "https://fieldwork.studio");
    snprintf(s.bio, sizeof s.bio, "Product designer in Berlin. Currently on onboarding and the settings pages.");
    s.timezone = 3;
    s.email_comments = s.email_mentions = true;
    s.push_mentions = true;
    s.font_scale = 1.0f;
    return cui_app_run(&(cui_app_desc){ .title = "Settings", .width = 1180, .height = 860 }, frame, &s);
}
