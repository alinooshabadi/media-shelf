/* The component gallery, shared by the gallery app and the screenshot tool. */
#ifndef GALLERY_UI_H
#define GALLERY_UI_H

#include <cui/cui.h>

enum {
    PAGE_OVERVIEW, PAGE_BUTTONS, PAGE_INPUTS, PAGE_SELECTION, PAGE_NAVIGATION,
    PAGE_OVERLAYS, PAGE_FEEDBACK, PAGE_DATA, PAGE_CHARTS, PAGE_COUNT
};

typedef struct gallery {
    int page;
    /* buttons */
    bool loading;
    int seg_view, seg_size;
    /* inputs */
    char name[64], email[96], password[64], search[64], bio[512], amount_text[32];
    double quantity;
    cui_date date;
    cui_color color;
    /* selection */
    bool terms, newsletter, marketing, wifi, bluetooth, airplane;
    cui_rect bluetooth_rect;   /* where the Bluetooth switch landed, so the smoke test can click it */
    int plan;
    float volume, brightness, lo, hi;
    int rating;
    bool chips[6];
    int framework;
    int plan_card;
    char tags[256];
    char otp[8];
    int country;
    /* navigation */
    int tab, tab2, page_index, step;
    /* overlays */
    bool dialog_open, sheet_open, palette_open, confirm_open;
    bool drawer;            /* the page list on a narrow window */
    /* data */
    bool acc[3];
    bool tree_src, tree_ui, tree_assets;
    int selected_row;
    cui_table_sort sort;
    int selected_file;
    int task_order[5];
    /* app */
    bool dark;
    int accent;
} gallery;

void gallery_init(gallery *g);
void gallery_frame(cui_ctx *ui, gallery *g);
const char *gallery_page_name(int page);
int gallery_page_total(void);   /* PAGE_COUNT plus the extended pages */

#endif
