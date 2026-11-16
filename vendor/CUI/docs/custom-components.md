# Custom components

When the built-in components are not enough, you can build your own with the same tools they use: boxes, a canvas with a 2D drawing API, per component state and springs.

## Compose first

Most "custom" components are compositions. A settings row:

```c
static bool setting_row(cui_ctx *ui, const char *title, const char *desc, bool *value) {
    bool changed = false;
    cui_push_id(ui, title);
    cui_row(ui, .gap = 16, .pad_y = 12) {
        cui_column(ui, .width = CUI_FILL, .gap = 2) {
            cui_label(ui, title);
            cui_text(ui, desc, .size = 12, .color = CUI_COLOR_TEXT_SECONDARY);
        }
        changed = cui_switch(ui, "##toggle", value);
    }
    cui_pop_id(ui);
    return changed;
}
```

## Canvas

`cui_canvas` reserves a box, reports interaction and calls your paint function with the final rectangle after layout.

```c
typedef struct { float level; } meter_data;

static void paint_meter(cui_ctx *ui, cui_rect r, void *user) {
    meter_data *d = user;
    cui_draw_rect(ui, r, CUI_CORNERS(r.h / 2), CUI_COLOR_SURFACE_MUTED);
    cui_rect fill = { r.x, r.y, r.w * d->level, r.h };
    cui_draw_rect(ui, fill, CUI_CORNERS(r.h / 2), CUI_COLOR_ACCENT);
}

meter_data d = { cui_animate(ui, cui_make_id(ui, "level"), level) };   /* smoothed */
cui_canvas_result c = cui_canvas(ui, "meter", CUI_FILL, CUI_PX(8), paint_meter, &d, sizeof d);
if (c.pressed) level = c.mouse.x / c.rect.w;
```

The user data is copied (pass `sizeof`), because painting happens at the end of the frame. `cui_canvas_result` has `rect`, `hovered`, `pressed`, `clicked` and `mouse` (relative to the canvas). The canvas clips to its rectangle.

## Drawing API

All coordinates are in points; CUI handles the display scale.

| Function | |
|---|---|
| `cui_draw_rect(ui, r, radius, color)` | Filled rectangle with per corner radii (`CUI_CORNERS(8)` or `(cui_corners){ tl, tr, br, bl }`) |
| `cui_draw_rect_border(ui, r, radius, fill, border, width)` | Fill and border in one shape |
| `cui_draw_rect_gradient(ui, r, radius, top, bottom)` | Vertical gradient |
| `cui_draw_shadow(ui, r, radius, blur, spread, offset, color)` | Gaussian shadow |
| `cui_draw_circle(ui, center, radius, color)` | |
| `cui_draw_line(ui, a, b, thickness, color)` | Anti-aliased |
| `cui_draw_polyline(ui, points, n, thickness, color, closed)` | With joins |
| `cui_draw_polygon(ui, points, n, color)` | Convex, anti-aliased |
| `cui_draw_arc(ui, center, radius, a0, a1, thickness, color)` | Angles in radians |
| `cui_draw_text(ui, font, size, pos, color, text, len)` | `len` -1 for the whole string |
| `cui_draw_icon(ui, CUI_ICON_*, rect, size, color)` | Centered in `rect` |
| `cui_draw_image(ui, texture, rect, uv, radius, tint)` | Rounded images too |
| `cui_push_clip(ui, r)` / `cui_pop_clip(ui)` | |
| `cui_measure_text(ui, font, size, text, len)` | Width, height and metrics |

Rounded rectangles and shadows are signed distance fields evaluated per pixel: corners are smooth at any size, and a border is part of the same shape, so there are no seams.

## State

Store whatever you need between frames under an id. The memory is zeroed on first use and freed a few seconds after the component stops appearing.

```c
typedef struct { int clicks; cui_spring wobble; } my_state;
my_state *st = cui_state(ui, cui_make_id(ui, "fancy"), sizeof *st);
st->clicks++;
```

## Animation

```c
float x = cui_animate(ui, id, target);                          /* smooth spring, no overshoot */
float y = cui_animate_spring(ui, id, target, 0.42f, 0.16f);     /* duration in seconds, bounce 0..1 */
```

Both request new frames while moving and stop when settled, so idle apps stay idle. Reduced motion makes them snap.

## Interaction model

For fully custom widgets, use the canvas result or `cui_last_hovered`. Hit testing uses the rectangles from the previous frame and honours overlays: a component under an open dialog or menu never sees the mouse. Keyboard focus, Tab order and the focus ring are handled for built-in components; custom canvases can read keys with `cui_key_pressed` while hovered or after a click.
