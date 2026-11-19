//! Ergonomic Rust layer over the raw CUI bindings.
//!
//! Conventions:
//! - Functions take the CUI context as `ui: *mut ffi::cui_ctx` (a Copy raw pointer).
//! - Strings are passed as `&str`; CUI copies them into per-frame memory, so the
//!   borrow only needs to live for the call.
//! - Option structs are the C layouts from `super::ffi`. Zero-initialize them
//!   (every field is optional) and set only what you need; the leading `__`
//!   padding field stays zero, which is the sentinel CUI checks.
//! - Scope helpers (box/row/column/scroll/dialog/...) run a closure between
//!   begin and end and guarantee `end` runs via a `Drop` guard, so a panic in
//!   the body cannot leak a half-open scope.

use super::ffi;
use core::ffi::{c_char, c_void};

/// The CUI context handle, passed to every component call.
pub type Ui = *mut ffi::cui_ctx;

// --------------------------------------------------------------------------- //
// Small literals (the C CUI_* macros)                                          //
// --------------------------------------------------------------------------- //

pub const fn fit() -> ffi::cui_size {
    ffi::cui_size { kind: ffi::cui_size_kind_CUI_SIZE_FIT, value: 0.0 }
}
pub const fn fill() -> ffi::cui_size {
    ffi::cui_size { kind: ffi::cui_size_kind_CUI_SIZE_GROW, value: 1.0 }
}
pub const fn px(v: f32) -> ffi::cui_size {
    ffi::cui_size { kind: ffi::cui_size_kind_CUI_SIZE_FIXED, value: v }
}
pub const fn pct(v: f32) -> ffi::cui_size {
    ffi::cui_size { kind: ffi::cui_size_kind_CUI_SIZE_PERCENT, value: v }
}

/// A CUI semantic token color (`CUI_COLOR_*`).
pub const fn color(token: ffi::cui_token) -> ffi::cui_color {
    ffi::token_color(token as u8)
}

// Common token shorthands.
pub const fn bg() -> ffi::cui_color { color(ffi::cui_token_CUI_TOKEN_BACKGROUND) }
pub const fn surface() -> ffi::cui_color { color(ffi::cui_token_CUI_TOKEN_SURFACE) }
pub const fn raised() -> ffi::cui_color { color(ffi::cui_token_CUI_TOKEN_SURFACE_RAISED) }
pub const fn muted() -> ffi::cui_color { color(ffi::cui_token_CUI_TOKEN_SURFACE_MUTED) }
pub const fn fg() -> ffi::cui_color { color(ffi::cui_token_CUI_TOKEN_FOREGROUND) }
pub const fn secondary() -> ffi::cui_color { color(ffi::cui_token_CUI_TOKEN_TEXT_SECONDARY) }
pub const fn text_muted() -> ffi::cui_color { color(ffi::cui_token_CUI_TOKEN_TEXT_MUTED) }
pub const fn border() -> ffi::cui_color { color(ffi::cui_token_CUI_TOKEN_BORDER) }
pub const fn border_strong() -> ffi::cui_color { color(ffi::cui_token_CUI_TOKEN_BORDER_STRONG) }
pub const fn accent() -> ffi::cui_color { color(ffi::cui_token_CUI_TOKEN_ACCENT) }
pub const fn accent_fg() -> ffi::cui_color { color(ffi::cui_token_CUI_TOKEN_ACCENT_FOREGROUND) }
pub const fn success() -> ffi::cui_color { color(ffi::cui_token_CUI_TOKEN_SUCCESS) }
pub const fn warning() -> ffi::cui_color { color(ffi::cui_token_CUI_TOKEN_WARNING) }
pub const fn danger() -> ffi::cui_color { color(ffi::cui_token_CUI_TOKEN_DANGER) }

// --------------------------------------------------------------------------- //
// Zero-initialization                                                         //
// --------------------------------------------------------------------------- //

/// Zero-initialize an option struct. All CUI option fields are optional and a
/// zeroed struct is the "use defaults" form; the leading padding field stays 0.
///
/// # Safety
/// `T` must be a C option struct with no fields that are invalid when zero.
/// Every `cui_*_opts` / `cui_box` / `cui_*_column` / `cui_size` / `cui_color`
/// in this crate satisfies that.
pub unsafe fn opts<T>() -> T {
    unsafe { core::mem::zeroed() }
}

/// NUL-terminated C string for the duration of one call. CUI copies the bytes
/// into per-frame storage before returning, so this is always safe to hand over.
pub fn cstr(s: &str) -> *const c_char {
    s.as_ptr().cast()
}

/// A CUI icon codepoint (`ffi::CUI_ICON_*` is a 4-byte UTF-8 `&[u8; 4]`).
pub fn icon(s: &[u8; 4]) -> *const c_char {
    s.as_ptr().cast()
}

// --------------------------------------------------------------------------- //
// Panic-safe scope guards                                                     //
// --------------------------------------------------------------------------- //

pub(crate) struct ScopeEnd {
    pub(crate) ui: Ui,
    pub(crate) kind: ScopeKind,
}
pub(crate) enum ScopeKind {
    None,
    Box,
    Card,
    Scroll,
    Split,
    Dialog,
    DialogFooter,
    Menu,
    ContextMenu,
    Tree,
    StatusBar,
}
impl Drop for ScopeEnd {
    fn drop(&mut self) {
        unsafe {
            match self.kind {
                ScopeKind::None => {}
                ScopeKind::Box => ffi::cui_box_end(self.ui),
                ScopeKind::Card => ffi::cui_card_end(self.ui),
                ScopeKind::Scroll => ffi::cui_scroll_end(self.ui),
                ScopeKind::Split => ffi::cui_split_end(self.ui),
                ScopeKind::Dialog => ffi::cui_dialog_end(self.ui),
                ScopeKind::DialogFooter => ffi::cui_dialog_footer_end(self.ui),
                ScopeKind::Menu => ffi::cui_menu_end(self.ui),
                ScopeKind::ContextMenu => ffi::cui_context_menu_end(self.ui),
                ScopeKind::Tree => ffi::cui_tree_end(self.ui),
                ScopeKind::StatusBar => ffi::cui_status_bar_end(self.ui),
            }
        }
    }
}

// --------------------------------------------------------------------------- //
// App lifecycle                                                               //
// --------------------------------------------------------------------------- //

/// Run the application: create the window, call `frame` until it closes, and
/// destroy it. `user` is passed to `frame` unchanged (hold your `App` there).
pub fn run(
    title: &str,
    width: i32,
    height: i32,
    min_width: i32,
    min_height: i32,
    follow_system_theme: bool,
    frame: extern "C" fn(Ui, *mut c_void),
    user: *mut c_void,
) -> i32 {
    let mut desc: ffi::cui_app_desc = unsafe { opts() };
    desc.title = cstr(title);
    desc.width = width;
    desc.height = height;
    desc.min_width = min_width;
    desc.min_height = min_height;
    desc.renderer = ffi::cui_renderer_kind_CUI_RENDERER_AUTO;
    desc.theme = ffi::cui_theme_mode_CUI_THEME_LIGHT;
    desc.accent = ffi::cui_accent_CUI_ACCENT_BLUE;
    desc.follow_system_theme = follow_system_theme;
    unsafe { ffi::cui_app_run(&desc, Some(frame), user) }
}

/// One-shot frame for a hidden/secondary context (used by tests).
pub fn begin_frame(ui: Ui, seconds: f32) {
    unsafe { ffi::cui_begin_frame(ui, seconds) }
}
pub fn end_frame(ui: Ui) {
    unsafe { ffi::cui_end_frame(ui) }
}

// --------------------------------------------------------------------------- //
// Repaint                                                                     //
// --------------------------------------------------------------------------- //

pub fn request_redraw(ui: Ui) {
    unsafe { ffi::cui_request_redraw(ui) }
}
/// Wake up after `seconds` to repaint (replace egui `request_repaint_after`).
pub fn request_wakeup(ui: Ui, seconds: f32) {
    unsafe { ffi::cui_request_wakeup(ui, seconds) }
}
pub fn needs_redraw(ui: Ui) -> bool {
    unsafe { ffi::cui_needs_redraw(ui) }
}
pub fn next_wakeup(ui: Ui) -> f32 {
    unsafe { ffi::cui_next_wakeup(ui) }
}

// --------------------------------------------------------------------------- //
// Theme                                                                       //
// --------------------------------------------------------------------------- //

pub fn set_theme(ui: Ui, dark: bool) {
    unsafe {
        ffi::cui_set_theme(
            ui,
            if dark {
                ffi::cui_theme_mode_CUI_THEME_DARK
            } else {
                ffi::cui_theme_mode_CUI_THEME_LIGHT
            },
        )
    }
}
pub fn set_reduced_motion(ui: Ui, reduced: bool) {
    unsafe { ffi::cui_set_reduced_motion(ui, reduced) }
}

// --------------------------------------------------------------------------- //
// Viewport / focus                                                            //
// --------------------------------------------------------------------------- //

pub fn viewport_size(ui: Ui) -> ffi::cui_vec2 {
    unsafe { ffi::cui_viewport_size(ui) }
}
pub fn wants_keyboard(ui: Ui) -> bool {
    unsafe { ffi::cui_wants_keyboard(ui) }
}

// --------------------------------------------------------------------------- //
// Identity                                                                    //
// --------------------------------------------------------------------------- //

pub fn push_id(ui: Ui, s: &str) {
    unsafe { ffi::cui_push_id(ui, cstr(s)) }
}
pub fn push_id_int(ui: Ui, v: i32) {
    unsafe { ffi::cui_push_id_int(ui, v) }
}
pub fn pop_id(ui: Ui) {
    unsafe { ffi::cui_pop_id(ui) }
}
pub fn push_disabled(ui: Ui, d: bool) {
    unsafe { ffi::cui_push_disabled(ui, d) }
}
pub fn pop_disabled(ui: Ui) {
    unsafe { ffi::cui_pop_disabled(ui) }
}
pub fn last_hovered(ui: Ui) -> bool {
    unsafe { ffi::cui_last_hovered(ui) }
}
pub fn tooltip(ui: Ui, text: &str) {
    unsafe { ffi::cui_tooltip(ui, cstr(text)) }
}

// --------------------------------------------------------------------------- //
// Layout containers                                                           //
// --------------------------------------------------------------------------- //

pub fn box_<F>(ui: Ui, opts: &ffi::cui_box, build: F)
where
    F: FnOnce(Ui),
{
    unsafe { ffi::cui_box_begin(ui, opts) };
    let _g = ScopeEnd { ui, kind: ScopeKind::Box };
    build(ui);
}
pub fn column<F>(ui: Ui, opts: &ffi::cui_box, build: F)
where
    F: FnOnce(Ui),
{
    unsafe { ffi::cui_column_begin(ui, opts) };
    let _g = ScopeEnd { ui, kind: ScopeKind::Box };
    build(ui);
}
pub fn row<F>(ui: Ui, opts: &ffi::cui_box, build: F)
where
    F: FnOnce(Ui),
{
    unsafe { ffi::cui_row_begin(ui, opts) };
    let _g = ScopeEnd { ui, kind: ScopeKind::Box };
    build(ui);
}
pub fn spacer(ui: Ui) {
    unsafe { ffi::cui_spacer(ui) }
}
pub fn gap(ui: Ui, size: f32) {
    unsafe { ffi::cui_gap(ui, size) }
}
pub fn separator(ui: Ui) {
    unsafe { ffi::cui_separator(ui) }
}

pub fn scroll<F>(ui: Ui, opts: &ffi::cui_scroll_opts, build: F)
where
    F: FnOnce(Ui),
{
    unsafe { ffi::cui_scroll_begin(ui, opts) };
    let _g = ScopeEnd { ui, kind: ScopeKind::Scroll };
    build(ui);
}
pub fn scroll_to(ui: Ui, id: &str, y: f32) {
    unsafe { ffi::cui_scroll_to(ui, cstr(id), y) }
}

/// Begin a scroll region, returning a guard that ends it on drop. Use when the
/// region must outlive the enclosing statement (e.g. a loop that draws into it).
pub(crate) fn scroll_begin(ui: Ui, opts: &ffi::cui_scroll_opts) -> ScopeEnd {
    unsafe { ffi::cui_scroll_begin(ui, opts) };
    ScopeEnd { ui, kind: ScopeKind::Scroll }
}

pub fn card<F>(ui: Ui, opts: &ffi::cui_card_opts, build: F) -> bool
where
    F: FnOnce(Ui),
{
    let clicked = unsafe { ffi::cui_card_begin(ui, opts) };
    let _g = ScopeEnd { ui, kind: ScopeKind::Card };
    build(ui);
    clicked
}

/// Begin a resizable two-pane split. Returns a guard that ends the split
/// scope on drop; call [`split_next`] between the two panes. `ratio` is read
/// and written in place.
pub(crate) fn split_begin(ui: Ui, id: &str, ratio: &mut f32, opts: &ffi::cui_split_opts) -> ScopeEnd {
    unsafe { ffi::cui_split_begin(ui, cstr(id), ratio, opts) };
    ScopeEnd { ui, kind: ScopeKind::Split }
}

/// Close pane one, draw the divider, and open pane two.
pub fn split_next(ui: Ui) {
    unsafe { ffi::cui_split_next(ui) };
}

// --------------------------------------------------------------------------- //
// Text                                                                        //
// --------------------------------------------------------------------------- //

pub fn text(ui: Ui, s: &str, opts: &ffi::cui_text_opts) {
    unsafe { ffi::cui_text_ex(ui, cstr(s), opts) }
}
pub fn heading(ui: Ui, s: &str, level: i32) {
    unsafe { ffi::cui_heading(ui, cstr(s), level) }
}
pub fn link(ui: Ui, s: &str) -> bool {
    unsafe { ffi::cui_link(ui, cstr(s)) }
}

// --------------------------------------------------------------------------- //
// Actions                                                                     //
// --------------------------------------------------------------------------- //

pub fn button(ui: Ui, label: &str, opts: &ffi::cui_button_opts) -> bool {
    unsafe { ffi::cui_button_ex(ui, cstr(label), opts) }
}
pub fn icon_button(ui: Ui, icon: &str, opts: &ffi::cui_button_opts) -> bool {
    unsafe { ffi::cui_icon_button_ex(ui, cstr(icon), opts) }
}
pub fn badge(ui: Ui, text: &str, opts: &ffi::cui_badge_opts) {
    unsafe { ffi::cui_badge_ex(ui, cstr(text), opts) }
}
pub fn segmented(
    ui: Ui,
    id: &str,
    items: &[&str],
    index: &mut i32,
    opts: &ffi::cui_segmented_opts,
) -> bool {
    let ptr = items.as_ptr() as *const *const c_char;
    unsafe { ffi::cui_segmented_ex(ui, cstr(id), ptr, items.len() as i32, index, opts) }
}
pub fn progress(ui: Ui, value: f32, opts: &ffi::cui_progress_opts) {
    unsafe { ffi::cui_progress_ex(ui, value, opts) }
}
pub fn spinner(ui: Ui, size: f32) {
    unsafe { ffi::cui_spinner(ui, size) }
}
pub fn skeleton(ui: Ui, width: ffi::cui_size, height: f32) {
    unsafe { ffi::cui_skeleton(ui, width, height) }
}
pub fn toast(ui: Ui, title: &str, opts: &ffi::cui_toast_opts) -> i32 {
    unsafe { ffi::cui_toast_ex(ui, cstr(title), opts) }
}
pub fn toast_action_clicked(ui: Ui, handle: i32) -> bool {
    unsafe { ffi::cui_toast_action_clicked(ui, handle) }
}
pub fn toast_dismiss(ui: Ui, handle: i32) {
    unsafe { ffi::cui_toast_dismiss(ui, handle) }
}

// --------------------------------------------------------------------------- //
// Inputs                                                                      //
// --------------------------------------------------------------------------- //

/// Single-line text field editing a Rust-owned byte buffer.
///
/// `buf` must have room for `cap` bytes including the NUL terminator. CUI keeps
/// the buffer NUL-terminated; read the text with [`input_as_str`].
/// CUI calls `strlen(buf)` on every edit control, so a zero-length Rust buffer
/// (whose `as_mut_ptr` is dangling) must be presented as a valid empty C
/// string instead. A zero-length buffer can never hold a change, so falling
/// back to a static `""` is lossless.
fn edit_buf(buf: &mut [u8]) -> *mut c_char {
    if buf.is_empty() {
        static EMPTY: [u8; 1] = [0];
        EMPTY.as_ptr().cast_mut().cast()
    } else {
        buf.as_mut_ptr().cast()
    }
}

/// Returns CUI event flags: [`ffi::CUI_CHANGED`], [`ffi::CUI_SUBMITTED`].
pub fn input(ui: Ui, id: &str, buf: &mut [u8], cap: usize, opts: &ffi::cui_input_opts) -> i32 {
    unsafe { ffi::cui_input_ex(ui, cstr(id), edit_buf(buf), cap as usize, opts) }
}
pub fn textarea(ui: Ui, id: &str, buf: &mut [u8], cap: usize, opts: &ffi::cui_input_opts) -> i32 {
    unsafe { ffi::cui_textarea_ex(ui, cstr(id), edit_buf(buf), cap as usize, opts) }
}
pub fn search(ui: Ui, id: &str, buf: &mut [u8], cap: usize, opts: &ffi::cui_input_opts) -> i32 {
    unsafe { ffi::cui_search_ex(ui, cstr(id), edit_buf(buf), cap as usize, opts) }
}
pub fn number(ui: Ui, id: &str, value: &mut f64, lo: f64, hi: f64, opts: &ffi::cui_number_opts) -> bool {
    unsafe { ffi::cui_number_ex(ui, cstr(id), value, lo, hi, opts) }
}
pub fn slider(ui: Ui, label: &str, value: &mut f32, lo: f32, hi: f32, opts: &ffi::cui_slider_opts) -> bool {
    unsafe { ffi::cui_slider_ex(ui, cstr(label), value, lo, hi, opts) }
}
pub fn checkbox(ui: Ui, label: &str, value: &mut bool, opts: &ffi::cui_check_opts) -> bool {
    unsafe { ffi::cui_checkbox_ex(ui, cstr(label), value, opts) }
}
pub fn switch(ui: Ui, label: &str, value: &mut bool, opts: &ffi::cui_check_opts) -> bool {
    unsafe { ffi::cui_switch_ex(ui, cstr(label), value, opts) }
}
pub fn select(
    ui: Ui,
    id: &str,
    items: &[&str],
    index: &mut i32,
    opts: &ffi::cui_select_opts,
) -> bool {
    let ptr = items.as_ptr() as *const *const c_char;
    unsafe { ffi::cui_select_ex(ui, cstr(id), ptr, items.len() as i32, index, opts) }
}

/// Read a CUI-editable byte buffer (NUL-terminated) as a Rust string.
pub fn input_as_str(buf: &[u8]) -> &str {
    let len = buf.iter().position(|&b| b == 0).unwrap_or(buf.len());
    // The buffer contents are exactly the bytes the user typed; they were
    // produced by CUI's UTF-8 text handling.
    core::str::from_utf8(&buf[..len]).unwrap_or("")
}

// --------------------------------------------------------------------------- //
// Selection / lists                                                           //
// --------------------------------------------------------------------------- //

pub fn list_item(ui: Ui, title: &str, opts: &ffi::cui_list_item_opts) -> bool {
    unsafe { ffi::cui_list_item_ex(ui, cstr(title), opts) }
}

// --------------------------------------------------------------------------- //
// Tabs                                                                        //
// --------------------------------------------------------------------------- //

pub fn tabs(
    ui: Ui,
    id: &str,
    items: &[&str],
    index: &mut i32,
    opts: &ffi::cui_tabs_opts,
) -> bool {
    let ptr = items.as_ptr() as *const *const c_char;
    unsafe { ffi::cui_tabs_ex(ui, cstr(id), ptr, items.len() as i32, index, opts) }
}

// --------------------------------------------------------------------------- //
// Data grid (virtualized table)                                               //
// --------------------------------------------------------------------------- //

/// Begin a virtualized grid; only rows in `[first, last)` are built.
/// Returns `true` and writes the visible range through `first`/`last` when the
/// grid body should be drawn this frame.
pub fn data_grid_begin(
    ui: Ui,
    id: &str,
    columns: &[ffi::cui_grid_column],
    row_count: i32,
    first: &mut i32,
    last: &mut i32,
    opts: &ffi::cui_grid_opts,
) -> bool {
    unsafe {
        ffi::cui_data_grid_begin_ex(ui, cstr(id), columns.as_ptr(), columns.len() as i32, row_count, first, last, opts)
    }
}
pub fn data_grid_row(ui: Ui, row: i32) -> bool {
    unsafe { ffi::cui_data_grid_row(ui, row) }
}
pub fn data_grid_cell(ui: Ui) {
    unsafe { ffi::cui_data_grid_cell(ui) }
}
pub fn data_grid_cell_text(ui: Ui, text: &str) {
    unsafe { ffi::cui_data_grid_cell_text(ui, cstr(text)) }
}
/// Grid cell with an explicit text color (e.g. warning tone for missing rows).
pub fn data_grid_cell_text_colored(ui: Ui, text: &str, color: ffi::cui_color) {
    unsafe { ffi::cui_data_grid_cell(ui) };
    let mut t = unsafe { core::mem::zeroed::<ffi::cui_text_opts>() };
    t.nowrap = true;
    t.line_height = 1.3;
    t.color = color;
    unsafe { ffi::cui_text_ex(ui, cstr(text), &t) }
}
pub fn data_grid_end(ui: Ui) {
    unsafe { ffi::cui_data_grid_end(ui) }
}

// --------------------------------------------------------------------------- //
// Tree                                                                        //
// --------------------------------------------------------------------------- //

/// Begin a tree node. `open` is read and written in place (expansion state).
/// Returns `true` when the node is expanded and the body should be built.
///
/// `cui_tree_end` is called only when the node is open — it pops the id stack
/// and the depth counter, which `cui_tree_begin` only pushes when open. This
/// mirrors CUI's `cui_tree` macro.
pub fn tree<F>(ui: Ui, label: &str, open: &mut bool, opts: &ffi::cui_tree_opts, build: F) -> bool
where
    F: FnOnce(Ui),
{
    let expanded = unsafe { ffi::cui_tree_begin(ui, cstr(label), open, opts) };
    if expanded {
        let _g = ScopeEnd { ui, kind: ScopeKind::Tree };
        build(ui);
    }
    expanded
}

/// `true` when the node from the last `cui_tree` call was clicked this frame.
pub fn tree_clicked(ui: Ui) -> bool {
    unsafe { ffi::cui_tree_clicked(ui) }
}

/// Begin a tree node. Returns `(is_open, guard)`: `is_open` is true while the
/// body should be drawn, and the guard ends the node scope on drop (a no-op
/// when collapsed). `open` toggles on click. Capture [`tree_clicked`]
/// immediately after this call — the next `tree_begin` overwrites it.
pub(crate) fn tree_begin(ui: Ui, label: &str, open: &mut bool, opts: &ffi::cui_tree_opts) -> (bool, ScopeEnd) {
    let expanded = unsafe { ffi::cui_tree_begin(ui, cstr(label), open, opts) };
    let kind = if expanded { ScopeKind::Tree } else { ScopeKind::None };
    (expanded, ScopeEnd { ui, kind })
}

/// End a tree node (pair with [`tree_begin`] when using it directly).
pub fn tree_end(ui: Ui) {
    unsafe { ffi::cui_tree_end(ui) }
}

/// Currently held modifier keys (`CUI_MOD_SHIFT`/`CTRL`/`ALT`/`SUPER`).
pub fn key_mods(ui: Ui) -> i32 {
    unsafe { ffi::cui_key_mods(ui) }
}

// --------------------------------------------------------------------------- //
// Dialog / menu / context menu                                                //
// --------------------------------------------------------------------------- //

/// Begin a dialog. `open` is read and written in place (the close button sets
/// it to false). Returns `true` when the dialog body should be drawn.
pub fn dialog<F>(ui: Ui, title: &str, open: &mut bool, opts: &ffi::cui_dialog_opts, build: F) -> bool
where
    F: FnOnce(Ui),
{
    let shown = unsafe { ffi::cui_dialog_begin(ui, cstr(title), open, opts) };
    if shown {
        let _g = ScopeEnd { ui, kind: ScopeKind::Dialog };
        build(ui);
    }
    shown
}
/// Begin the action footer of the current dialog.
pub fn dialog_footer<F>(ui: Ui, build: F)
where
    F: FnOnce(Ui),
{
    unsafe { ffi::cui_dialog_footer_begin(ui) };
    let _g = ScopeEnd { ui, kind: ScopeKind::DialogFooter };
    build(ui);
}

/// Begin a labeled dropdown menu. Returns `true` when open.
pub fn menu<F>(ui: Ui, label: &str, opts: &ffi::cui_menu_opts, build: F) -> bool
where
    F: FnOnce(Ui),
{
    let open = unsafe { ffi::cui_menu_begin(ui, cstr(label), opts) };
    if open {
        let _g = ScopeEnd { ui, kind: ScopeKind::Menu };
        build(ui);
    }
    open
}
/// Begin a context menu for the last component. Returns `true` when open.
pub fn context_menu<F>(ui: Ui, id: &str, build: F) -> bool
where
    F: FnOnce(Ui),
{
    let open = unsafe { ffi::cui_context_menu_begin(ui, cstr(id)) };
    if open {
        let _g = ScopeEnd { ui, kind: ScopeKind::ContextMenu };
        build(ui);
    }
    open
}
pub fn menu_item(ui: Ui, label: &str, opts: &ffi::cui_menu_item_opts) -> bool {
    unsafe { ffi::cui_menu_item_ex(ui, cstr(label), opts) }
}
pub fn menu_separator(ui: Ui) {
    unsafe { ffi::cui_menu_separator(ui) }
}
pub fn menu_label(ui: Ui, text: &str) {
    unsafe { ffi::cui_menu_label(ui, cstr(text)) }
}

// --------------------------------------------------------------------------- //
// Status bar                                                                  //
// --------------------------------------------------------------------------- //

pub fn status_bar<F>(ui: Ui, id: &str, opts: &ffi::cui_status_bar_opts, build: F)
where
    F: FnOnce(Ui),
{
    unsafe { ffi::cui_status_bar_begin(ui, cstr(id), opts) };
    let _g = ScopeEnd { ui, kind: ScopeKind::StatusBar };
    build(ui);
}
pub fn status_bar_spacer(ui: Ui) {
    unsafe { ffi::cui_status_bar_spacer(ui) }
}
pub fn status_bar_text(ui: Ui, text: &str, opts: &ffi::cui_status_item_opts) {
    unsafe { ffi::cui_status_bar_text_ex(ui, cstr(text), opts) }
}
pub fn status_bar_item(ui: Ui, label: &str, opts: &ffi::cui_status_item_opts) -> bool {
    unsafe { ffi::cui_status_bar_item_ex(ui, cstr(label), opts) }
}
pub fn status_bar_progress(ui: Ui, value: f32, opts: &ffi::cui_status_progress_opts) {
    unsafe { ffi::cui_status_bar_progress_ex(ui, value, opts) }
}

// --------------------------------------------------------------------------- //
// Images                                                                      //
// --------------------------------------------------------------------------- //

pub fn image_load(ui: Ui, path: &str) -> (ffi::cui_texture, i32, i32) {
    let mut w: i32 = 0;
    let mut h: i32 = 0;
    let tex = unsafe { ffi::cui_image_load(ui, cstr(path), &mut w, &mut h) };
    (tex, w, h)
}
pub fn image_create(ui: Ui, w: i32, h: i32, rgba: &[u8]) -> ffi::cui_texture {
    unsafe { ffi::cui_image_create(ui, w, h, rgba.as_ptr().cast::<c_void>()) }
}
pub fn image(ui: Ui, tex: ffi::cui_texture, opts: &ffi::cui_image_opts) {
    unsafe { ffi::cui_image_ex(ui, tex, opts) }
}
pub fn image_destroy(ui: Ui, tex: ffi::cui_texture) {
    unsafe { ffi::cui_image_destroy(ui, tex) }
}

// --------------------------------------------------------------------------- //
// Platform services                                                           //
// --------------------------------------------------------------------------- //

/// Open a native file/folder picker (blocking). Writes the chosen path (NUL
/// terminated) into `out` and returns `true` when the user confirmed.
pub fn file_dialog(opts: &ffi::cui_file_dialog_opts, out: &mut [u8]) -> bool {
    unsafe {
        ffi::cui_app_file_dialog(
            core::ptr::null_mut(),
            opts,
            out.as_mut_ptr().cast(),
            out.len() as usize,
        )
    }
}
/// Open a URL or file with the system default handler.
pub fn open_url(url: &str) -> bool {
    unsafe { ffi::cui_app_open_url(cstr(url)) }
}
pub fn app_quit() {
    unsafe { ffi::cui_app_quit(core::ptr::null_mut()) }
}

// --------------------------------------------------------------------------- //
// Keyboard shortcut                                                           //
// --------------------------------------------------------------------------- //

/// Returns `true` this frame when the given key + modifiers were pressed.
pub fn shortcut(ui: Ui, mods: ffi::cui_mod, key: ffi::cui_key) -> bool {
    unsafe { ffi::cui_shortcut(ui, mods as i32, key) }
}

// --------------------------------------------------------------------------- //
// Fonts                                                                       //
// --------------------------------------------------------------------------- //

pub fn font_load_file(ui: Ui, path: &str) -> i32 {
    unsafe { ffi::cui_font_load_file(ui, cstr(path)) }
}
pub fn font_set_role(ui: Ui, role: i32, font: i32) {
    unsafe { ffi::cui_font_set_role(ui, role, font) }
}
