/* Headless direction, mirror, and pointer regression; optional FONT OUT_PREFIX. */
#include "../src/cui_internal.h"
#include <cui/cui_render.h>

#define W 480
#define H 700
#define CHECK(x, msg) do { if (!(x)) { fprintf(stderr, "%s at %d\n", msg, __LINE__); exit(1); } } while (0)
static int clicked[6];
static cui_id button_ids[6];
static bool dark;
#ifdef CUI_TEXT_SHAPING
static const bool shaping = true;
#else
static const bool shaping = false;
#endif
static bool enabled = true, checked = true;
static int segment;
static char input[64];
static const char *search_placeholder = "جست‌وجو در کارها";
static const char *titles[] = {"سلام دنیا", "Review changes", "امروز ۳ تا Timer دارم", "Timer: سلام ۱۲۳ دنیا."};
static const char *lines[] = {"سه تا کار مونده", "Ready for review", "زمان رو بررسی کن", "Ready at 12:30"};

static void screen(cui_ctx *ui, bool automatic) {
    cui_column(ui, .id = "panel", .width = CUI_FILL, .height = CUI_FILL, .pad = 24, .gap = 16) {
        cui_row(ui, .id = "header", .width = CUI_FILL, .height = CUI_PX(40)) {
            cui_text(ui, automatic ? "Today's work" : "کارهای امروز", .size = 24, .nowrap = true);
            cui_spacer(ui);
            if (cui_icon_button(ui, CUI_ICON_CHEVRON_RIGHT, .tooltip = "Header")) clicked[0]++;
            button_ids[0] = cui_last_id(ui);
        }
        cui_column(ui, .id = "list", .width = CUI_FILL, .gap = 8) {
            for (int k = 0; k < 4; k++) {
                cui_push_id_int(ui, k);
                cui_row(ui, .id = "row", .width = CUI_FILL, .height = CUI_PX(64), .gap = 12,
                        .direction = automatic ? CUI_DIRECTION_AUTO : CUI_DIRECTION_INHERIT) {
                    cui_box(ui, .width = CUI_PX(24), .height = CUI_PX(24), .radius = 12,
                            .bg = CUI_C(ui, CONTROL_ON), .dir = CUI_STACK, .align = CUI_ALIGN_CENTER, .justify = CUI_JUSTIFY_CENTER) {
                        cui_icon(ui, CUI_ICON_CHECK, 14, CUI_C(ui, CONTROL_GLYPH));
                    }
                    cui_column(ui, .id = "copy", .width = CUI_FILL, .gap = 0) {
                        cui_text(ui, titles[k], .size = 16, .nowrap = true, .line_height = 1.5f);
                        cui_text(ui, lines[k], .size = 12, .nowrap = true, .line_height = 1.5f, .color = CUI_COLOR_TEXT_SECONDARY);
                    }
                    if (cui_icon_button(ui, CUI_ICON_CHEVRON_RIGHT, .tooltip = "Open")) clicked[k+1]++;
                    button_ids[k+1] = cui_last_id(ui);
                }
                cui_pop_id(ui);
            }
        }
        const char *fa[] = {"امروز", "هفته", "همه"};
        const char *en[] = {"Today", "Week", "All"};
        cui_segmented(ui, "period", automatic ? en : fa, 3, &segment, .full_width = true);
        cui_input(ui, "search", input, sizeof input, .placeholder = search_placeholder, .icon = CUI_ICON_SEARCH);
        cui_switch(ui, automatic ? "Notifications" : "اعلان‌ها", &enabled);
        cui_checkbox(ui, automatic ? "Keep open" : "باز بمونه", &checked);
        cui_progress(ui, 0.4f, .height = 8);
        cui_row(ui, .id = "footer", .width = CUI_FILL, .gap = 8, .justify = CUI_JUSTIFY_END) {
            cui_button(ui, automatic ? "Cancel" : "بی‌خیال", .variant = CUI_BUTTON_OUTLINE);
            if (cui_button(ui, automatic ? "Continue" : "ادامه", .variant = CUI_BUTTON_PRIMARY, .icon_right = CUI_ICON_ARROW_RIGHT)) clicked[5]++;
            button_ids[5] = cui_last_id(ui);
        }
    }
}
static void frames(cui_ctx *ui, bool automatic, int count) {
    for (int f=0; f<count; f++) { cui_begin_frame(ui, 1.0f/60); screen(ui, automatic); cui_end_frame(ui); }
}
static void alignment_cases(cui_ctx *ui) {
    cui_begin_frame(ui,1.0f/60);
    cui_column(ui,.width=CUI_FILL,.pad_left=12,.pad_right=28,.gap=4) {
        for(int type=0;type<4;type++) {
            cui_box(ui,.dir=(cui_dir)type,.width=CUI_PX(180),.height=CUI_PX(80),.pad_left=8,.pad_right=20,
                    .gap=8,.align=CUI_ALIGN_END,.justify=CUI_JUSTIFY_END,.columns=2,.wrap=type==CUI_ROW) {
                for(int k=0;k<3;k++) cui_box(ui,.width=CUI_PX(48),.height=CUI_PX(20)) {}
            }
        }
        cui_row(ui,.direction=CUI_DIRECTION_AUTO,.width=CUI_PX(180)) {cui_text(ui,"123 … سلام",.nowrap=true);}
        cui_row(ui,.direction=CUI_DIRECTION_AUTO,.width=CUI_PX(180)) {cui_text(ui,"123 … Hello",.nowrap=true);}
        cui_row(ui,.direction=CUI_DIRECTION_AUTO,.width=CUI_PX(180)) {cui_text(ui,"123 …",.nowrap=true);}
    }
    cui_end_frame(ui);
}
static cui_ctx *context(cui_sw_renderer *sw, cui_direction d, const char *font) {
    cui_ctx *ui = cui_create(&(cui_ctx_desc){.renderer=cui_sw_iface(sw),.theme=dark?CUI_THEME_DARK:CUI_THEME_LIGHT,.direction=d});
    cui_set_viewport(ui,W,H,1);
    cui_set_reduced_motion(ui,true);
    if (font) {
        cui_font ft=cui_font_load_file(ui,font); CHECK(ft>=0,"font load failed");
        cui_font_set_role(ui,CUI_FONT_BODY,ft); cui_font_set_role(ui,CUI_FONT_MEDIUM,ft); cui_font_set_role(ui,CUI_FONT_DISPLAY,ft);
    }
    return ui;
}
static void save(cui_ctx *ui, cui_sw_renderer *sw, const char *prefix, const char *suffix) {
    char path[1024]; snprintf(path,sizeof path,"%s-%s.png",prefix,suffix);
    const cui_draw_data *dd=cui_get_draw_data(ui);
    unsigned char *px=malloc(W*H*4); CHECK(px,"pixels allocation failed");
    cui_sw_render(sw,dd,px,W,H,W*4,CUI_SW_RGBA); CHECK(cui_write_png(path,px,W,H,W*4),"PNG failed"); free(px);
    snprintf(path,sizeof path,"%s-%s.json",prefix,suffix); FILE *out=fopen(path,"w"); CHECK(out,"JSON failed");
    fprintf(out,"[\n");
    for(int i=0;i<ui->node_count;i++) {
        cui_node *n=&ui->nodes[i];
        fprintf(out,"{\"node\":%d,\"parent\":%d,\"rtl\":%d,\"x\":%.4f,\"y\":%.4f,\"w\":%.4f,\"h\":%.4f}%s\n",i,n->parent,n->rtl,n->rect.x,n->rect.y,n->rect.w,n->rect.h,i+1<ui->node_count?",":"");
    }
    fprintf(out,"]\n"); fclose(out);
}
static void left_alignment(cui_sw_renderer *sw) {
    cui_ctx *ui = context(sw, CUI_DIRECTION_INHERIT, NULL);
    cui_begin_frame(ui, 1.0f/60);
    cui_row(ui, .width = CUI_FILL, .height = CUI_PX(80)) {
        cui_text(ui, "same", .nowrap = true);
        cui_text(ui, "same", .nowrap = true, .align = CUI_TEXT_LEFT);
    }
    cui_end_frame(ui);
    cui_rect a = ui->nodes[2].rect, b = ui->nodes[3].rect;
    CHECK(fabsf(a.y - b.y) < 0.001f && fabsf(a.h - b.h) < 0.001f && b.h < 80,
          "physical LEFT must retain upstream row centering");
    printf("explicit LEFT: y=%.6f h=%.6f, same as default\n", b.y, b.h);
    cui_destroy(ui);
}
static void scroll_frame(cui_ctx *ui) {
    cui_begin_frame(ui, 1.0f/60);
    cui_row(ui, .id = "overflow", .width = CUI_PX(100), .height = CUI_PX(40), .scroll_x = true) {
        for (int k = 0; k < 3; k++) cui_box(ui, .width = CUI_PX(80), .height = CUI_PX(30)) {}
    }
    cui_end_frame(ui);
}
static void scroll_overflow(cui_sw_renderer *sw) {
    for (int rtl = 0; rtl < 2; rtl++) {
        cui_ctx *ui = context(sw, rtl ? CUI_DIRECTION_RTL : CUI_DIRECTION_LTR, NULL);
        scroll_frame(ui); scroll_frame(ui);
        cui_node *n = &ui->nodes[1];
        cui_rect view = n->rect;
        float start = ui->nodes[2].rect.x;
        CHECK(fabsf(n->content_w - 240) < 0.001f, "scroll content extent lost");
        cui_input_mouse_move(ui, view.x + 50, view.y + 20);
        cui_input_scroll(ui, -4, 0); scroll_frame(ui);
        cui_scroll_state *st = cui_state(ui, ui->nodes[1].id, sizeof *st);
        CHECK(st->tx == 140 && st->x == 140, "wheel cannot reach overflow");
        CHECK(fabsf(ui->nodes[2].rect.x - start - (rtl ? 140 : -140)) < 0.001f,
              "horizontal scroll moved in wrong direction");
        cui_rect last = ui->nodes[4].rect;
        CHECK(last.x >= view.x && last.x + last.w <= view.x + view.w, "last child unreachable");
        cui_input_scroll(ui, 4, 0); scroll_frame(ui);
        CHECK(st->x == 0 && st->tx == 0 && ui->nodes[2].rect.x == start, "cannot scroll back to start");
        cui_destroy(ui);
    }
    printf("LTR/RTL overflow: content 240, viewport 100, wheel reaches 140 and returns to 0\n");
}
static void popover_direction(cui_sw_renderer *sw) {
    for (int mode = 0; mode < 3; mode++) {
        cui_ctx *ui = context(sw, mode == 1 ? CUI_DIRECTION_RTL : CUI_DIRECTION_LTR, NULL);
        cui_begin_frame(ui, 1.0f/60);
        cui_column(ui, .direction = mode == 0 ? CUI_DIRECTION_RTL : mode == 1 ? CUI_DIRECTION_LTR : CUI_DIRECTION_AUTO,
                   .width = CUI_FILL) {
            cui_text(ui, "سلام");
            cui_popover_open(ui, "popup");
            cui_popover(ui, "popup", .width = 180) { cui_text(ui, "Hello"); }
        }
        cui_end_frame(ui);
        CHECK(ui->float_count == 1, "popover missing");
        cui_node *popup = &ui->nodes[ui->float_roots[0]];
        CHECK(popup->rtl == (mode == 0 || (shaping && mode == 2)) && ui->nodes[popup->first].rtl == popup->rtl,
              "popover must inherit opening container direction");
        cui_destroy(ui);
    }
    printf("popover direction: explicit RTL, explicit LTR and resolved AUTO inherited\n");
}
static void placeholder_direction(cui_sw_renderer *sw, const char *font) {
    size_t bytes = W * H * 4;
    unsigned char *pixels = malloc(bytes * 2); CHECK(pixels, "placeholder pixels");
    for (int rtl = 0; rtl < 2; rtl++) {
        for (int root_rtl = 0; root_rtl < 2; root_rtl++) {
            cui_ctx *ui = context(sw, root_rtl ? CUI_DIRECTION_RTL : CUI_DIRECTION_LTR, font);
            char empty[2] = "";
            cui_begin_frame(ui, 1.0f/60);
            cui_column(ui, .direction = rtl ? CUI_DIRECTION_RTL : CUI_DIRECTION_LTR, .width = CUI_FILL) {
                cui_input(ui, "placeholder", empty, sizeof empty, .placeholder = "Timer: سلام 123.", .width = CUI_FILL);
            }
            cui_end_frame(ui);
            cui_sw_render(sw, cui_get_draw_data(ui), pixels + bytes * root_rtl, W, H, W*4, CUI_SW_RGBA);
            cui_destroy(ui);
        }
        CHECK(!memcmp(pixels, pixels + bytes, bytes), "placeholder used context instead of container direction");
    }
    free(pixels);
    printf("LTR/RTL placeholders: 0 differing pixels between opposite context directions\n");
}
static void intrinsic_direction(cui_sw_renderer *sw, const char *font) {
    for (int rtl = 0; rtl < 2; rtl++) for (int wrap = 0; wrap < 2; wrap++) {
        cui_ctx *ui = context(sw, rtl ? CUI_DIRECTION_LTR : CUI_DIRECTION_RTL, font);
        const char *text = "T. سلام";
        for (int frame = 0; frame < 2; frame++) {
            cui_begin_frame(ui, 1.0f/60);
            cui_row(ui, .direction = rtl ? CUI_DIRECTION_RTL : CUI_DIRECTION_LTR, .width = CUI_FILL) {
                cui_text(ui, text, .size = 32, .nowrap = !wrap);
            }
            cui_end_frame(ui);
            cui_node *n = &ui->nodes[2];
            cui_ctx *reference = context(sw, rtl ? CUI_DIRECTION_RTL : CUI_DIRECTION_LTR, font);
            float width = cui_measure_text(reference, CUI_FONT_BODY, 32, text, -1).width;
            float minimum = cui__text_min_width(reference, CUI_FONT_BODY, 32, text, (int)strlen(text));
            CHECK(fabsf(n->pref_w - ceilf(width*4)/4) < .001f, "intrinsic width used context instead of resolved node direction");
            if (wrap) CHECK(fabsf(n->minc_w - ceilf(minimum*4)/4) < .001f, "minimum width used context instead of resolved node direction");
            cui_destroy(reference);
        }
        cui_destroy(ui);
    }
    printf("intrinsic widths: opposite context, LTR/RTL, FIT/nowrap and wrap, repeated frames passed\n");
}
static void split_frame(cui_ctx *ui, float *ratio, bool vertical, cui_direction direction) {
    cui_begin_frame(ui, 1.0f/60);
    cui_column(ui, .direction = direction, .width = CUI_PX(500), .height = CUI_PX(500)) {
        cui_split(ui, "split", ratio, .vertical = vertical, .min_first = 70, .min_second = 110) {
            cui_text(ui, "سلام");
            cui_split_next(ui);
            cui_text(ui, "Second");
        }
    }
    cui_end_frame(ui);
}
static float divider_position(cui_ctx *ui, bool vertical) {
    cui_rect r = cui__prev_rect(ui, cui_make_id(ui, "split") ^ 0x78u);
    return vertical ? r.y + r.h/2 : r.x + r.w/2;
}
static void split_direction(cui_sw_renderer *sw) {
    const cui_direction directions[] = { CUI_DIRECTION_INHERIT, CUI_DIRECTION_RTL, CUI_DIRECTION_LTR, CUI_DIRECTION_AUTO };
    for (int root_rtl = 0; root_rtl < 2; root_rtl++) for (int mode = 0; mode < 4; mode++) for (int vertical = 0; vertical < 2; vertical++) {
        cui_ctx *ui = context(sw, root_rtl ? CUI_DIRECTION_RTL : CUI_DIRECTION_LTR, NULL);
        cui_set_viewport(ui, 600, 600, 1);
        float ratio = .3f;
        for (int f = 0; f < 3; f++) split_frame(ui, &ratio, vertical, directions[mode]);
        float start = divider_position(ui, vertical);
        float x = vertical ? 250 : start, y = vertical ? start : 250;
        cui_input_mouse_move(ui, x, y); split_frame(ui, &ratio, vertical, directions[mode]);
        cui_input_mouse_button(ui, CUI_MOUSE_LEFT, true); split_frame(ui, &ratio, vertical, directions[mode]);
        CHECK(fabsf(ratio - .3f) < .002f, "split divider jumps on press");
        CHECK(fabsf(divider_position(ui, vertical) - start) < 1, "split moved without pointer movement");
        cui_input_mouse_move(ui, x + (vertical ? 0.0f : 40.0f), y + (vertical ? 40.0f : 0.0f));
        split_frame(ui, &ratio, vertical, directions[mode]);
        CHECK(fabsf(divider_position(ui, vertical) - start - 40) < 1, "split divider moves against pointer");
        bool rtl = directions[mode] == CUI_DIRECTION_RTL || (directions[mode] == CUI_DIRECTION_INHERIT && root_rtl);
#ifdef CUI_TEXT_SHAPING
        rtl |= directions[mode] == CUI_DIRECTION_AUTO;
#endif
        CHECK((ratio < .3f) == (rtl && !vertical), "split ratio used wrong origin");
        cui_input_mouse_move(ui, vertical ? 250.0f : 0.0f, vertical ? 0.0f : 250.0f);
        split_frame(ui, &ratio, vertical, directions[mode]);
        CHECK(fabsf(ratio - (rtl && !vertical ? .78f : .14f)) < .001f, "split minimum sizes use wrong direction");
        cui_input_mouse_button(ui, CUI_MOUSE_LEFT, false); split_frame(ui, &ratio, vertical, directions[mode]);
        cui_destroy(ui);
    }
    printf("split press, drag and minimums: LTR/RTL/inherited/AUTO, horizontal/vertical passed\n");
}
static void column_alignment(cui_sw_renderer *sw) {
    const cui_text_align aligns[] = { CUI_TEXT_START, CUI_TEXT_END, CUI_TEXT_LEFT, CUI_TEXT_RIGHT, CUI_TEXT_CENTER };
    const cui_direction directions[] = { CUI_DIRECTION_INHERIT, CUI_DIRECTION_RTL, CUI_DIRECTION_LTR, CUI_DIRECTION_AUTO };
    for (int root_rtl = 0; root_rtl < 2; root_rtl++) for (int mode = 0; mode < 4; mode++) for (int grid = 0; grid < 2; grid++) {
        cui_ctx *ui = context(sw, root_rtl ? CUI_DIRECTION_RTL : CUI_DIRECTION_LTR, NULL);
        cui_set_viewport(ui, 800, 400, 1);
        const char *title = directions[mode] == CUI_DIRECTION_AUTO ? "سلام" : "Header";
        cui_table_column table_cols[5]; cui_grid_column grid_cols[5];
        memset(table_cols, 0, sizeof table_cols); memset(grid_cols, 0, sizeof grid_cols);
        for (int k = 0; k < 5; k++) {
            table_cols[k].title = grid_cols[k].title = title;
            table_cols[k].align = grid_cols[k].align = aligns[k];
            table_cols[k].width = CUI_PX(140); grid_cols[k].width = 140; grid_cols[k].fixed = true;
            table_cols[k].sortable = grid_cols[k].sortable = true;
        }
        cui_table_sort sort = {0};
        int first, last;
        cui_begin_frame(ui, 1.0f/60);
        cui_column(ui, .direction = directions[mode], .width = CUI_PX(750), .height = CUI_PX(300)) {
            if (grid) {
                CHECK(cui_data_grid_begin_ex(ui, "grid", grid_cols, 5, 1, &first, &last, &(cui_grid_opts){ .height = CUI_PX(200), .sort = &sort }), "grid missing");
                cui_data_grid_row(ui, 0);
                for (int k = 0; k < 5; k++) cui_data_grid_cell_text(ui, "Cell");
                cui_data_grid_end(ui);
            } else {
                CHECK(cui_table_begin_ex(ui, "table", table_cols, 5, &(cui_table_opts){ .sort = &sort }), "table missing");
                cui_table_row(ui, false);
                for (int k = 0; k < 5; k++) cui_table_cell_text(ui, "Cell");
                cui_table_end(ui);
            }
        }
        cui_end_frame(ui);
        int header = 0, body = 0;
        bool rtl = directions[mode] == CUI_DIRECTION_RTL || (directions[mode] == CUI_DIRECTION_INHERIT && root_rtl);
#ifdef CUI_TEXT_SHAPING
        rtl |= directions[mode] == CUI_DIRECTION_AUTO;
#endif
        for (int i = 0; i < ui->node_count; i++) {
            cui_node *text = &ui->nodes[i];
            if (!(text->flags & CUI_NF_TEXT) || text->font == CUI_FONT_ICONS) continue;
            bool heading = !strcmp(text->text, title);
            if (!heading && strcmp(text->text, "Cell")) continue;
            int k = heading ? header++ : body++;
            cui_node *cell = &ui->nodes[text->parent];
            float left = text->rect.x, right = left + text->rect.w;
            for (int c = cell->first; c >= 0; c = ui->nodes[c].next) {
                left = CUI_MIN(left, ui->nodes[c].rect.x);
                right = CUI_MAX(right, ui->nodes[c].rect.x + ui->nodes[c].rect.w);
            }
            bool right_aligned = aligns[k] == CUI_TEXT_RIGHT || (aligns[k] == CUI_TEXT_START && rtl) || (aligns[k] == CUI_TEXT_END && !rtl);
            float expected = aligns[k] == CUI_TEXT_CENTER ? cell->rect.x + (cell->rect.w - (right-left))/2 :
                right_aligned ? cell->rect.x + cell->rect.w - cell->pad[1] - (right-left) : cell->rect.x + cell->pad[3];
            CHECK(cell->rtl == rtl, "column inherited wrong direction");
            CHECK(fabsf(left-expected) < .01f, "table/grid header or cell violated physical/logical alignment");
        }
        CHECK(header == 5 && body == 5, "column alignment coverage missing");
        cui_destroy(ui);
    }
    printf("table/grid headers (with sort icons) and cells: all five alignments, inherited/explicit/AUTO passed\n");
}
/* Code, numbers, times and key combinations keep left to right inside an RTL screen, and
 * so do a week calendar and a gantt (their grids are drawn by hand). Markdown paragraphs
 * and left-aligned table columns stay at the left in LTR. */
static void keep_ltr(cui_sw_renderer *sw) {
    cui_ctx *ui = context(sw, CUI_DIRECTION_RTL, NULL);
    static char code[8] = "418";
    int first[4] = {0};
    for (int f = 0; f < 2; f++) {
        cui_begin_frame(ui, 1.0f/60);
        cui_column(ui, .width = CUI_FILL) {
            first[0] = ui->node_count; cui_kbd(ui, "Ctrl Shift P");
            first[1] = ui->node_count; cui_counter(ui, "money", 48210, .group_thousands = true);
            first[2] = ui->node_count; cui_otp_input(ui, "otp", code, 6);
            first[3] = ui->node_count; cui_code_block(ui, "code", "int x = 1;");
            cui_row(ui) { cui_text(ui, "a"); cui_text(ui, "b"); }
        }
        cui_end_frame(ui);
    }
    for (int k = 0; k < 4; k++) CHECK(!ui->nodes[first[k]].rtl, "component should keep LTR in an RTL screen");
    /* keycaps run Ctrl, Shift, P from the left */
    int caps[3], nc = 0;
    for (int c = ui->nodes[first[0]].first; c >= 0 && nc < 3; c = ui->nodes[c].next) caps[nc++] = c;
    CHECK(nc == 3 && ui->nodes[caps[0]].rect.x < ui->nodes[caps[1]].rect.x && ui->nodes[caps[1]].rect.x < ui->nodes[caps[2]].rect.x, "keycaps out of order");
    /* while an ordinary row still mirrors */
    int row = ui->node_count - 3;
    CHECK(ui->nodes[row].rtl && ui->nodes[row + 1].rect.x > ui->nodes[row + 2].rect.x, "plain row should mirror");
    cui_destroy(ui);

    /* Markdown in LTR: a paragraph and a left column start at the left edge */
    ui = context(sw, CUI_DIRECTION_LTR, NULL);
    for (int f = 0; f < 2; f++) {
        cui_begin_frame(ui, 1.0f/60);
        cui_column(ui, .width = CUI_FILL, .pad = 0) { cui_markdown(ui, "md", "Hi\n\n| a | b |\n|---|--:|\n| x | y |"); }
        cui_end_frame(ui);
    }
    unsigned char *px = malloc(W*H*4); CHECK(px, "pixels allocation failed");
    cui_sw_render(sw, cui_get_draw_data(ui), px, W, H, W*4, CUI_SW_RGBA);
    int left = W;   /* leftmost dark pixel of the first text line */
    for (int y = 0; y < 30; y++) for (int x = 0; x < W; x++) if (px[(y*W + x)*4] < 128 && x < left) left = x;
    CHECK(left < 40, "Markdown paragraph is not at the left");
    free(px);
    cui_destroy(ui);
    printf("LTR kept: keycaps, counter, code input, code block; Markdown paragraph at x=%d\n", left);
}
/* Sliders and radio cards read and answer the pointer and arrow keys the way they are drawn. */
static float slider_value = 50;
static int card = 0;
static void controls_frame(cui_ctx *ui) {
    static const cui_choice choices[] = { { "One", NULL, NULL, NULL }, { "Two", NULL, NULL, NULL }, { "Three", NULL, NULL, NULL } };
    cui_begin_frame(ui, 1.0f/60);
    cui_column(ui, .width = CUI_FILL, .pad = 20, .gap = 20) {
        cui_slider(ui, "Volume", &slider_value, 0, 100);
        cui_radio_cards(ui, "cards", choices, 3, &card, .columns = 3);
    }
    cui_end_frame(ui);
}
static void rtl_controls(cui_sw_renderer *sw) {
    for (int d = 0; d < 2; d++) {
        bool rtl = d == 1;
        cui_ctx *ui = context(sw, rtl ? CUI_DIRECTION_RTL : CUI_DIRECTION_LTR, NULL);
        slider_value = 50; card = 0;
        for (int f = 0; f < 3; f++) controls_frame(ui);
        cui_rect tr = cui__prev_rect(ui, cui_make_id(ui, "Volume") ^ 0x51u);
        /* press near the right end of the track */
        cui_input_mouse_move(ui, tr.x + tr.w - 12, tr.y + tr.h / 2); controls_frame(ui);
        cui_input_mouse_button(ui, CUI_MOUSE_LEFT, true); controls_frame(ui);
        cui_input_mouse_button(ui, CUI_MOUSE_LEFT, false); controls_frame(ui);
        CHECK(rtl ? slider_value < 10 : slider_value > 90, "slider pointer direction");
        /* the slider has focus: Left moves toward the left end of the track */
        float before = slider_value;
        cui_input_key(ui, CUI_KEY_LEFT, true, 0); controls_frame(ui); cui_input_key(ui, CUI_KEY_LEFT, false, 0); controls_frame(ui);
        CHECK(rtl ? slider_value > before : slider_value < before, "slider arrow direction");
        /* radio cards: focus the first with Tab and press the arrow toward the second card */
        cui_rect c0 = cui__prev_rect(ui, cui__hash(&(int){ 0 }, sizeof(int), cui_make_id(ui, "cards")));
        cui_rect c1 = cui__prev_rect(ui, cui__hash(&(int){ 1 }, sizeof(int), cui_make_id(ui, "cards")));
        if (fabsf(c0.y - c1.y) < 1) CHECK(rtl ? c1.x < c0.x : c1.x > c0.x, "radio cards order");   /* side by side when wide enough */
        cui_input_key(ui, CUI_KEY_TAB, true, 0); controls_frame(ui); cui_input_key(ui, CUI_KEY_TAB, false, 0); controls_frame(ui);
        cui_key toward = rtl ? CUI_KEY_LEFT : CUI_KEY_RIGHT;
        cui_input_key(ui, toward, true, 0); controls_frame(ui); cui_input_key(ui, toward, false, 0); controls_frame(ui);
        CHECK(card == 1, "radio card arrow direction");
        cui_destroy(ui);
    }
    printf("RTL controls: slider pointer and arrows, radio card arrows\n");
}
int main(int argc,char **argv) {
    dark=argc>3;
    cui_sw_renderer *sw=cui_sw_create();
    column_alignment(sw);
    split_direction(sw);
    intrinsic_direction(sw, argc>1?argv[1]:NULL);
    left_alignment(sw); scroll_overflow(sw); popover_direction(sw); keep_ltr(sw); rtl_controls(sw);
    placeholder_direction(sw, argc>1?argv[1]:NULL);
    cui_ctx *ltr=context(sw,CUI_DIRECTION_LTR,argc>1?argv[1]:NULL);
    cui_ctx *rtl=context(sw,CUI_DIRECTION_RTL,argc>1?argv[1]:NULL);
    frames(ltr,false,3); frames(rtl,false,3);
    CHECK(ltr->node_count==rtl->node_count,"tree differs");
    for (int i=0;i<ltr->node_count;i++) {
        cui_rect a=ltr->nodes[i].rect,b=rtl->nodes[i].rect;
        if(fabsf(b.x-(W-a.x-a.w))>0.5f || fabsf(b.y-a.y)>0.5f || fabsf(b.w-a.w)>0.5f || fabsf(b.h-a.h)>0.5f) {
            fprintf(stderr,"mirror failed node %d LTR %.2f %.2f %.2f %.2f RTL %.2f %.2f %.2f %.2f\n",i,a.x,a.y,a.w,a.h,b.x,b.y,b.w,b.h);return 1;
        }
    }
    printf("mirror passed: all %d node rectangles within 0.5 px\n",ltr->node_count);
    int buttons=0;
    for(int k=0;k<6;k++) {
        cui_rect b=cui__prev_rect(rtl,button_ids[k]);
        int before=clicked[k];
        cui_input_mouse_move(rtl,b.x+b.w/2,b.y+b.h/2); frames(rtl,false,1);
        cui_input_mouse_button(rtl,CUI_MOUSE_LEFT,true);frames(rtl,false,1);
        cui_input_mouse_button(rtl,CUI_MOUSE_LEFT,false);frames(rtl,false,1);
        CHECK(clicked[k]==before+1,"click triggered the wrong button");buttons++;
    }
    CHECK(buttons==6,"RTL button click failed"); printf("RTL clicks passed: header, four rows, footer action\n");
    cui_input_mouse_leave(rtl);frames(rtl,false,2);
    if(argc>2) {save(ltr,sw,argv[2],"ltr");save(rtl,sw,argv[2],"rtl");}
    cui_ctx *automatic=context(sw,CUI_DIRECTION_LTR,argc>1?argv[1]:NULL);frames(automatic,true,3);
    int rows=0;
    for(int i=0;i<automatic->node_count;i++) if(automatic->nodes[i].direction==CUI_DIRECTION_AUTO) {
        bool expected=shaping&&(rows==0||rows==2);
        CHECK(automatic->nodes[i].rtl==expected,"AUTO first strong failed");
        for(int j=i+1;j<automatic->node_count && automatic->nodes[j].parent>=i;j++) CHECK(automatic->nodes[j].rtl==expected,"row subtitle direction differs");
        rows++;
    }
    CHECK(rows==4,"AUTO rows missing");printf("AUTO passed: %s\n",shaping?"first strong":"LTR without shaping");
    if(argc>2)save(automatic,sw,argv[2],"auto");
    alignment_cases(ltr);alignment_cases(rtl);
    for(int i=0;i<ltr->node_count;i++) {
        cui_node *a=&ltr->nodes[i],*b=&rtl->nodes[i];
        if(a->parent<0 || ltr->nodes[a->parent].direction!=CUI_DIRECTION_AUTO)
            CHECK(fabsf(b->rect.x-(W-a->rect.x-a->rect.w))<=0.5f,"alignment/padding mirror failed");
        if(a->direction==CUI_DIRECTION_AUTO) {
            if(i==ltr->node_count-2) CHECK(!a->rtl&&b->rtl==shaping,"neutral AUTO direction");
        }
    }
    CHECK(ltr->nodes[ltr->node_count-6].rtl==shaping&&rtl->nodes[rtl->node_count-6].rtl==shaping,"AUTO Persian direction");
    CHECK(!ltr->nodes[ltr->node_count-4].rtl&&!rtl->nodes[rtl->node_count-4].rtl,"AUTO must skip digits before English");
    printf("asymmetric padding, end alignment, wrapped row, grid, stack and neutral AUTO passed\n");
    cui_destroy(ltr);cui_destroy(rtl);cui_destroy(automatic);cui_sw_destroy(sw);return 0;
}
