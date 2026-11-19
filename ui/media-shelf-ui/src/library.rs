//! The movies/series library list, rendered as a virtualized CUI data grid.
//!
//! `SortColumn`, `title`, `year` and the row provider are ported from the egui
//! view; the table itself is a `cui_data_grid` (only visible rows are built).

use crate::app::App;
use crate::ffi;
use media_shelf::{Category, Video};
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum SortColumn {
    Title,
    Year,
    Rating,
    Status,
    Seasons,
    Episodes,
    Added,
}

/// Display title: provider identity → NFO identity → local guess.
pub fn title(v: &Video) -> String {
    v.tmdb
        .as_ref()
        .map(|m| m.title.clone())
        .or_else(|| v.metadata.as_ref().map(|m| m.identity.title.clone()))
        .unwrap_or_else(|| {
            if v.category == Category::Movies {
                return media_shelf::identification::local_identity(v).0;
            }
            v.path
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned()
        })
}

fn year(v: &Video) -> String {
    v.metadata
        .as_ref()
        .and_then(|m| m.year)
        .map(|y| y.to_string())
        .or_else(|| {
            v.tmdb
                .as_ref()
                .map(|m| m.year().to_string())
                .filter(|y| y != "Unknown year")
        })
        .or_else(|| {
            if v.category == Category::Movies {
                media_shelf::identification::local_identity(v)
                    .1
                    .map(|y| y.to_string())
            } else {
                None
            }
        })
        .unwrap_or_default()
}

fn status_label(v: &Video) -> &'static str {
    if v.metadata.is_some() {
        "Scraped"
    } else if v.tmdb.is_some() {
        "Matched"
    } else {
        "New"
    }
}

fn rating_label(v: &Video) -> String {
    v.metadata
        .as_ref()
        .and_then(|m| m.rating)
        .filter(|r| r.is_finite())
        .map(|r| format!("{r:.1}"))
        .unwrap_or_default()
}

fn optional<T: Ord>(a: Option<T>, b: Option<T>, desc: bool) -> std::cmp::Ordering {
    match (a, b) {
        (Some(a), Some(b)) => {
            if desc {
                b.cmp(&a)
            } else {
                a.cmp(&b)
            }
        }
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (Some(_), None) => std::cmp::Ordering::Less,
        _ => std::cmp::Ordering::Equal,
    }
}

/// The indices of the videos currently shown, filtered and sorted.
pub fn rows(app: &App) -> Vec<usize> {
    let filter = crate::ui::input_as_str(&app.filter).to_lowercase();
    let entries: Vec<usize> = app
        .videos
        .iter()
        .enumerate()
        .filter(|(_, v)| {
            v.category == app.category
                && (filter.is_empty()
                    || title(v).to_lowercase().contains(&filter)
                    || v.path.to_string_lossy().to_lowercase().contains(&filter))
                && match app.filter_mode {
                    1 => v.metadata.is_none(),
                    2 => v.artwork.poster.is_none()
                        && v
                            .metadata
                            .as_ref()
                            .map_or(true, |m| m.poster_file.is_none()),
                    _ => true,
                }
        })
        .map(|(i, _)| i)
        .collect();
    let mut entries = entries;
    entries.sort_by(|&a, &b| {
        let av = &app.videos[a];
        let bv = &app.videos[b];
        let count = |v: &Video| {
            app.series_tree.shows.get(&v.path).map(|s| {
                if app.sort == SortColumn::Seasons {
                    s.season_count()
                } else {
                    s.count()
                }
            })
        };
        let order = match app.sort {
            SortColumn::Title => {
                optional(Some(title(av).to_lowercase()), Some(title(bv).to_lowercase()), app.sort_descending)
            }
            SortColumn::Year => optional(
                Some(year(av)).filter(|s| !s.is_empty()),
                Some(year(bv)).filter(|s| !s.is_empty()),
                app.sort_descending,
            ),
            SortColumn::Rating => optional(
                av.metadata
                    .as_ref()
                    .and_then(|m| m.rating)
                    .filter(|r| r.is_finite())
                    .map(|r| (r * 1000.0) as i64),
                bv.metadata
                    .as_ref()
                    .and_then(|m| m.rating)
                    .filter(|r| r.is_finite())
                    .map(|r| (r * 1000.0) as i64),
                app.sort_descending,
            ),
            SortColumn::Status => optional(
                Some((av.metadata.is_some(), av.tmdb.is_some())),
                Some((bv.metadata.is_some(), bv.tmdb.is_some())),
                app.sort_descending,
            ),
            SortColumn::Added => {
                optional(av.added_date.as_ref(), bv.added_date.as_ref(), app.sort_descending)
            }
            SortColumn::Seasons | SortColumn::Episodes => {
                optional(count(av), count(bv), app.sort_descending)
            }
        };
        order
            .then_with(|| title(av).to_lowercase().cmp(&title(bv).to_lowercase()))
            .then_with(|| av.path.cmp(&bv.path))
    });
    entries
}

/// Map a grid column index to the sort column it drives (movies grid order).
fn grid_column_to_sort(index: i32) -> SortColumn {
    match index {
        1 => SortColumn::Year,
        2 => SortColumn::Rating,
        3 => SortColumn::Status,
        4 => SortColumn::Added,
        _ => SortColumn::Title,
    }
}

fn sort_to_grid_column(c: SortColumn) -> i32 {
    match c {
        SortColumn::Year => 1,
        SortColumn::Rating => 2,
        SortColumn::Status => 3,
        SortColumn::Added => 4,
        _ => 0,
    }
}

const FILTERS: [&str; 3] = ["All", "Unscraped", "No artwork"];

/// The library pane (left side): shared search/filter bar, item count, and
/// either the movies data grid or the series browsing tree.
pub fn library_pane(ui: crate::ui::Ui, app: &mut App) {
    use crate::ui as ui2;

    let mut pane = unsafe { ui2::opts::<ffi::cui_box>() };
    pane.dir = ffi::cui_dir_CUI_COLUMN;
    pane.width = ui2::fill();
    pane.height = ui2::fill();
    pane.pad = 16.0;
    pane.gap = 10.0;
    ui2::box_(ui, &pane, |ui| {
        // Toolbar: search + filter mode.
        let mut bar = unsafe { ui2::opts::<ffi::cui_box>() };
        bar.dir = ffi::cui_dir_CUI_ROW;
        bar.gap = 10.0;
        bar.align = ffi::cui_align_CUI_ALIGN_CENTER;
        ui2::box_(ui, &bar, |ui| {
            let mut sopts = unsafe { ui2::opts::<ffi::cui_input_opts>() };
            sopts.placeholder = ui2::cstr("Search library…");
            sopts.icon = ui2::icon(ffi::CUI_ICON_SEARCH);
            sopts.width = ui2::fill();
            let cap = app.filter.len() + 1;
            let flags = ui2::search(ui, "filter", &mut app.filter, cap, &sopts);
            if flags & ffi::CUI_CHANGED as i32 != 0 {
                ui2::request_redraw(ui);
            }
            let mut selopts = unsafe { ui2::opts::<ffi::cui_select_opts>() };
            selopts.width = ui2::px(120.0);
            if ui2::select(ui, "filter_mode", &FILTERS, &mut app.filter_mode, &selopts) {
                ui2::request_redraw(ui);
            }
        });

        // Selection count + buttons.
        let entries = rows(app);
        let mut count_bar = unsafe { ui2::opts::<ffi::cui_box>() };
        count_bar.dir = ffi::cui_dir_CUI_ROW;
        count_bar.gap = 10.0;
        count_bar.align = ffi::cui_align_CUI_ALIGN_CENTER;
        ui2::box_(ui, &count_bar, |ui| {
            let mut topts = unsafe { ui2::opts::<ffi::cui_text_opts>() };
            topts.color = ui2::text_muted();
            ui2::text(ui, &format!("{} items", entries.len()), &topts);
            ui2::spacer(ui);
            let mut bopts = unsafe { ui2::opts::<ffi::cui_button_opts>() };
            bopts.size = ffi::cui_control_size_CUI_SIZE_SM;
            if ui2::button(ui, "Select visible", &bopts) {
                app.selection =
                    entries.iter().map(|i| app.videos[*i].path.clone()).collect();
                app.selected = entries.first().map(|i| app.videos[*i].path.clone());
                app.selected_season = None;
                ui2::request_redraw(ui);
            }
            let mut bopts = unsafe { ui2::opts::<ffi::cui_button_opts>() };
            bopts.size = ffi::cui_control_size_CUI_SIZE_SM;
            bopts.variant = ffi::cui_button_variant_CUI_BUTTON_GHOST;
            if ui2::button(ui, "Clear selection", &bopts) {
                app.selection.clear();
                app.selected = None;
                app.selected_season = None;
                ui2::request_redraw(ui);
            }
        });

        ui2::gap(ui, 4.0);
        if app.category == Category::Series {
            series_tree(ui, app, &entries);
        } else {
            movies_grid(ui, app, &entries);
        }
    });
}

/// The virtualized movies data grid (left pane, movies category).
pub fn movies_grid(ui: crate::ui::Ui, app: &mut App, entries: &[usize]) {
    use crate::ui as ui2;

    let n = entries.len() as i32;
    let mut selected: Vec<bool> = entries
        .iter()
        .map(|i| app.selection.contains(&app.videos[*i].path))
        .collect();
    let mut sort = ffi::cui_table_sort {
        column: sort_to_grid_column(app.sort),
        descending: app.sort_descending,
    };
    let mut cursor: Option<i32> = app
        .selected
        .as_ref()
        .and_then(|p| entries.iter().position(|&i| &app.videos[i].path == p))
        .map(|i| i as i32);
    let cursor_ref = cursor.as_mut();

    let mut gopts = unsafe { ui2::opts::<ffi::cui_grid_opts>() };
    gopts.width = ui2::fill();
    gopts.height = ui2::fill();
    gopts.row_height = 44.0;
    gopts.striped = true;
    gopts.selected = selected.as_mut_ptr();
    gopts.sort = &mut sort;
    if let Some(c) = cursor_ref {
        gopts.cursor = c;
    }

    let cols: [&str; 5] = ["Title", "Year", "Rating", "Status", "Added"];
    let mut columns: Vec<ffi::cui_grid_column> = vec![];
    for (i, label) in cols.iter().enumerate() {
        let mut col: ffi::cui_grid_column = unsafe { ui2::opts() };
        col.title = ui2::cstr(label);
        col.sortable = true;
        col.align = ffi::cui_text_align_CUI_TEXT_START;
        col.width = match i {
            0 => 0.0,   // fill
            1 => 60.0,  // year
            2 => 70.0,  // rating
            3 => 84.0,  // status
            _ => 104.0, // added
        };
        if i == 0 {
            col.priority = -1; // title never hides
        }
        columns.push(col);
    }
    columns[0].width = 0.0;

    let mut first: i32 = 0;
    let mut last: i32 = 0;
    if ui2::data_grid_begin(
        ui, "movies", &columns, n, &mut first, &mut last, &gopts,
    ) {
        for r in first..last {
            let e = entries[r as usize];
            let v = &app.videos[e];
            if ui2::data_grid_row(ui, r) {
                app.selected = Some(v.path.clone());
                app.selected_season = None;
                ui2::request_redraw(ui);
            }
            ui2::data_grid_cell_text(ui, &title(v));
            ui2::data_grid_cell_text(ui, &year(v));
            ui2::data_grid_cell_text(ui, &rating_label(v));
            ui2::data_grid_cell_text(ui, status_label(v));
            ui2::data_grid_cell_text(ui, v.added_date.as_deref().unwrap_or("–"));
        }
        ui2::data_grid_end(ui);
    }

    // Write the grid's selection state back.
    app.selection = entries
        .iter()
        .zip(selected.iter())
        .filter(|(_, sel)| **sel)
        .map(|(&i, _)| app.videos[i].path.clone())
        .collect();
    app.sort = grid_column_to_sort(sort.column);
    app.sort_descending = sort.descending;
    if let Some(c) = cursor {
        if c >= 0 && (c as usize) < entries.len() {
            app.selected = Some(app.videos[entries[c as usize]].path.clone());
        }
    }
}

/// The series browsing tree (left pane, series category). Placeholder until the
/// full tree is ported.
/// The series browsing tree (left pane, series category).
///
/// One tree node per show folder; expanding a show requests an episode scan
/// on a background thread and reveals season nodes, episode rows, and missing
/// episodes. Right-clicking a show node opens its context menu (search &
/// scrape, reload local metadata, rename).
pub fn series_tree(ui: crate::ui::Ui, app: &mut App, entries: &[usize]) {
    use crate::ui as ui2;

    if entries.is_empty() {
        let mut t = unsafe { ui2::opts::<ffi::cui_text_opts>() };
        t.color = ui2::text_muted();
        ui2::text(ui, "No TV shows. Update sources to add some.", &t);
        return;
    }

    let mut scroll = unsafe { ui2::opts::<ffi::cui_scroll_opts>() };
    scroll.width = ui2::fill();
    scroll.height = ui2::fill();
    scroll.id = ui2::cstr("series_tree");
    scroll.pad = 6.0;
    let _scroll_guard = ui2::scroll_begin(ui, &scroll);

    let primary = ui2::key_mods(ui) & MOD_PRIMARY != 0;
    let mut season_clicked: Option<(PathBuf, u32)> = None;
    let mut episode_clicked: Option<(PathBuf, bool)> = None;
    let mut refresh: Option<PathBuf> = None;

    for &i in entries {
        // Copy everything we need out of the video up front so the loop body
        // can mutably borrow `app` (selection, context-menu actions) freely.
        let v = &app.videos[i];
        let folder = v.path.clone();
        let name = title(v);
        let year = v
            .metadata
            .as_ref()
            .and_then(|m| m.year)
            .map(|y| y.to_string())
            .unwrap_or_else(|| "–".into());
        let added = v.added_date.clone().unwrap_or_else(|| "–".into());
        let catalog = app.episode_lookup(v);
        let selected = app.selected.as_ref() == Some(&folder);

        let mut topts = unsafe { ui2::opts::<ffi::cui_tree_opts>() };
        topts.selected = selected;
        let open = app.show_open.entry(folder.clone()).or_insert(false);
        let (show_open_now, _show_guard) = ui2::tree_begin(ui, &name, open, &topts);
        let show_clicked = ui2::tree_clicked(ui);

        // Anchor the context menu to the show node (ui->last_id is the row
        // drawn by tree_begin above).
        if ui2::context_menu(ui, &ctx_id("show", &folder), |ui| {
            show_context_menu(ui, app, &folder);
        }) {
            // Opened: the chosen action is read back below.
        }
        let action = app.pending_show_action.take();

        if show_clicked {
            app.select(folder.clone(), primary);
        }
        if let Some(a) = action {
            apply_show_action(app, &folder, a);
        }

        if !show_open_now {
            // Collapsed show: nothing more to draw for it.
            continue;
        }

        // ---- Counts row ----------------------------------------------------
        let counts = app
            .series_tree
            .shows
            .get(&folder)
            .map(|s| s.count());

        let mut counts_row = unsafe { ui2::opts::<ffi::cui_box>() };
        counts_row.dir = ffi::cui_dir_CUI_ROW;
        counts_row.gap = 10.0;
        counts_row.pad_left = 26.0;
        ui2::box_(ui, &counts_row, |ui| {
            let mut t = unsafe { ui2::opts::<ffi::cui_text_opts>() };
            t.color = ui2::secondary();
            t.width = ui2::fill();
            let n = counts.map_or_else(|| "loading…".into(), |e| format!("{e} episodes"));
            ui2::text(ui, &n, &t);
            for cell in [&year, &added] {
                let mut t = unsafe { ui2::opts::<ffi::cui_text_opts>() };
                t.color = ui2::text_muted();
                t.nowrap = true;
                ui2::text(ui, cell, &t);
            }
        });

        // ---- Lazy episode load ---------------------------------------------
        app.series_tree.request(
            &folder,
            &app.sources,
            catalog,
            app.show_missing_episodes,
        );

        let loaded = match app.series_tree.shows.get(&folder) {
            Some(s) => s,
            None => {
                ui2::gap(ui, 4.0);
                ui2::spinner(ui, 14.0);
                ui2::gap(ui, 4.0);
                let mut t = unsafe { ui2::opts::<ffi::cui_text_opts>() };
                t.color = ui2::text_muted();
                ui2::text(ui, "Loading episodes…", &t);
                continue;
            }
        };

        if loaded.count() == 0 && loaded.warnings.is_empty() {
            let mut t = unsafe { ui2::opts::<ffi::cui_text_opts>() };
            t.color = ui2::text_muted();
            ui2::text(ui, "No video files found.", &t);
        }
        if loaded.checking_metadata {
            ui2::spinner(ui, 12.0);
            ui2::gap(ui, 4.0);
            let msg = if app.show_missing_episodes {
                "Checking missing episodes on TMDB…"
            } else {
                "Loading episode details from TMDB…"
            };
            let mut t = unsafe { ui2::opts::<ffi::cui_text_opts>() };
            t.color = ui2::text_muted();
            ui2::text(ui, msg, &t);
        }

        // ---- Season nodes -----------------------------------------------------
        for (season, episodes) in &loaded.seasons {
            let Some(season_num) = season else { continue };
            let missing = loaded.missing.get(season_num);
            let missing_count = missing.map_or(0, |m| m.len());
            let base_name = if *season_num == 0 { "Specials" } else { "Season" };
            let name = if episodes.is_empty() && missing_count > 0 {
                format!("{base_name} {season_num} · Missing")
            } else if missing_count > 0 {
                format!("{base_name} {season_num}  ·  {} local / {missing_count} missing", episodes.len())
            } else {
                format!("{base_name} {season_num}  ·  {} episodes", episodes.len())
            };
            let season_selected = app
                .selected_season
                .as_ref()
                .is_some_and(|(f, s)| f == &folder && s == season_num);

            let mut sopts = unsafe { ui2::opts::<ffi::cui_tree_opts>() };
            sopts.selected = season_selected;
            let open = app
                .season_open
                .entry((folder.clone(), *season_num))
                .or_insert(true);
            let (season_open_now, _season_guard) = ui2::tree_begin(ui, &name, open, &sopts);
            let clicked = ui2::tree_clicked(ui);
            if clicked && season_open_now {
                season_clicked = Some((folder.clone(), *season_num));
            }
            if !season_open_now {
                continue;
            }

            // ---- Episode rows --------------------------------------------------
            let mut rows: Vec<(u32, Option<&crate::series::Episode>, Option<&String>)> =
                episodes
                    .iter()
                    .map(|e| (e.number.unwrap_or(u32::MAX), Some(e), None))
                    .collect();
            if let Some(missing_map) = missing {
                rows.extend(missing_map.iter().map(|(n, t)| (*n, None, Some(t))));
            }
            rows.sort_by_key(|(n, _, _)| *n);
            for (number, episode, missing_title) in rows {
                match (episode, missing_title) {
                    (Some(e), _) => {
                        ui2::push_id(ui, &e.video.path.display().to_string());
                        let mut lopts = unsafe { ui2::opts::<ffi::cui_list_item_opts>() };
                        lopts.selected = app.selection.contains(&e.video.path);
                        let clicked = ui2::list_item(ui, &e.label, &lopts);
                        ui2::pop_id(ui);
                        if clicked {
                            episode_clicked = Some((e.video.path.clone(), primary));
                        }
                    }
                    (None, Some(title)) => {
                        ui2::push_id(ui, &format!("missing-{season_num}-{number}"));
                        ui2::gap(ui, 2.0);
                        let mut t = unsafe { ui2::opts::<ffi::cui_text_opts>() };
                        t.color = ui2::warning();
                        ui2::text(ui, &format!("{number}. {title} · Missing"), &t);
                        ui2::gap(ui, 2.0);
                        ui2::pop_id(ui);
                    }
                    _ => {}
                }
            }
        }

        // ---- Warnings + refresh ------------------------------------------------
        if !loaded.warnings.is_empty() {
            let mut t = unsafe { ui2::opts::<ffi::cui_text_opts>() };
            t.color = ui2::danger();
            ui2::text(ui, "Episode check needs attention.", &t);
        }
        if ui2::button(
            ui,
            "Refresh episodes",
            &crate::app::opts_button(ffi::cui_button_variant_CUI_BUTTON_GHOST, false),
        ) {
            refresh = Some(folder.clone());
        }
    }

    drop(_scroll_guard);

    // ---- Apply deferred interactions (all reads done) ----------------------------
    if let Some((folder, season)) = season_clicked {
        app.select_season(folder, season);
    }
    if let Some((path, additive)) = episode_clicked {
        app.select(path, additive);
        app.detail_tab = 2; // jump to the "Media files" tab
    }
    if let Some(folder) = refresh {
        app.series_tree.shows.remove(&folder);
        if let Some(v) = app.videos.iter().find(|v| v.path == folder) {
            app.series_tree.request(
                &folder,
                &app.sources,
                app.episode_lookup(v),
                app.show_missing_episodes,
            );
        }
    }
}

/// The "primary" modifier: Command on macOS, Ctrl elsewhere (CUI defines it as
/// a C preprocessor macro, so it is not in the bindgen output).
pub const MOD_PRIMARY: i32 = {
    #[cfg(target_os = "macos")]
    {
        8 // CUI_MOD_SUPER
    }
    #[cfg(not(target_os = "macos"))]
    {
        2 // CUI_MOD_CTRL
    }
};

/// What a show context menu can ask the app to do.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ShowAction {
    Scrape,
    ReloadLocal,
    Rename,
}

fn apply_show_action(app: &mut App, folder: &Path, action: ShowAction) {
    match action {
        ShowAction::Scrape => app.toast_msg = Some(
            "TMDB matching is on the way — this opens the scrape dialog.".into(),
        ),
        ShowAction::ReloadLocal => app.reload_show(folder),
        ShowAction::Rename => app.toast_msg = Some("Rename & cleanup is on the way.".into()),
    }
}

fn show_context_menu(ui: crate::ui::Ui, app: &mut App, _folder: &Path) {
    use crate::ui as ui2;
    let locked = app.items_locked();
    if ui2::menu_item(ui, "Search & scrape…", &crate::app::menu_item_opts(false, locked)) {
        app.pending_show_action = Some(ShowAction::Scrape);
    }
    if ui2::menu_item(ui, "Reload local metadata", &crate::app::menu_item_opts(false, locked)) {
        app.pending_show_action = Some(ShowAction::ReloadLocal);
    }
    ui2::menu_separator(ui);
    if ui2::menu_item(ui, "Rename…", &crate::app::menu_item_opts(false, locked)) {
        app.pending_show_action = Some(ShowAction::Rename);
    }
}

/// Unique context-menu id per show folder.
fn ctx_id(prefix: &str, folder: &Path) -> String {
    format!("{prefix}_{}", folder.display())
}

/// Show folders are browsed as a tree (see the series view), not a grid.
pub fn category_is_series(app: &App) -> bool {
    app.category == Category::Series
}

/// Open the folder containing `path` in the system file manager.
pub fn open_folder(path: &Path) -> Option<String> {
    if !path.is_dir() {
        return Some("Folder is unavailable. Check that the drive is connected.".into());
    }
    #[cfg(windows)]
    let result = {
        use std::os::windows::process::CommandExt;
        let explorer = PathBuf::from(std::env::var_os("WINDIR").unwrap_or_else(|| "C:\\Windows".into())).join("explorer.exe");
        std::process::Command::new(explorer)
            .arg(path)
            .creation_flags(0x08000000)
            .spawn()
    };
    #[cfg(target_os = "macos")]
    let result = std::process::Command::new("/usr/bin/open").arg(path).spawn();
    #[cfg(all(unix, not(target_os = "macos")))]
    let result = std::process::Command::new("xdg-open").arg(path).spawn();
    result.err().map(|e| format!("Could not open folder: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::App;
    use media_shelf::{Category, artwork::Artwork, metadata, tmdb};
    use std::{
        collections::HashSet,
        sync::{
            Arc,
            atomic::AtomicBool,
        },
    };

    fn video(category: Category, path: &str, tmdb: Option<tmdb::TmdbMatch>, metadata: Option<metadata::Metadata>) -> Video {
        Video {
            added_date: Some("2026-10-22".into()),
            is_folder: true,
            category,
            path: path.into(),
            relative_path: path.into(),
            bytes: 0,
            artwork: Artwork::default(),
            tmdb,
            metadata,
        }
    }

    /// Build an App for tests without opening the real library database.
    fn app(videos: Vec<Video>, category: Category) -> App {
        App {
            sources: vec![],
            category,
            folder_input: vec![],
            filter: vec![],
            videos,
            series_tree: Default::default(),
            warnings: vec![],
            error: None,
            status: String::new(),
            receiver: None,
            cancel: Arc::new(AtomicBool::new(false)),
            database: None,
            selected: None,
            selected_season: None,
            selection: HashSet::new(),
            detail_tab: 0,
            sources_open: false,
            filter_mode: 0,
            sort: SortColumn::Title,
            sort_descending: false,
            theme_dark: None,
            split_ratio: 0.34,
            poster_cache: Default::default(),
            show_open: Default::default(),
            season_open: Default::default(),
            toast_msg: None,
            pending_show_action: None,
            settings_open: false,
            show_missing_episodes: false,
        }
    }

    #[test]
    fn columns_sort_both_directions_and_unknown_values_stay_last() {
        let identity = tmdb::TmdbMatch {
            category: Category::Series,
            id: 42,
            title: "Alpha".into(),
            original_title: "Alpha".into(),
            release_date: Some("2020-01-01".into()),
            overview: String::new(),
        };
        let mut metadata = metadata::Metadata::empty(identity.clone());
        metadata.year = Some(2020);
        metadata.rating = Some(9.0);
        let a = video(Category::Series, "/shows/Alpha", Some(identity), Some(metadata));
        let b = video(Category::Series, "/shows/Zulu", None, None);
        let mut app = app(vec![a, b], Category::Series);
        app.sort = SortColumn::Rating;
        app.sort_descending = false;
        let order = rows(&app);
        assert_eq!(order, vec![0, 1]);
        app.sort_descending = true;
        assert_eq!(rows(&app), vec![0, 1], "unknown ratings always last");
    }

    #[test]
    fn title_prefers_provider_then_local() {
        let v = video(
            Category::Movies,
            "/movies/Dune (2021)/Dune.mp4",
            Some(tmdb::TmdbMatch {
                category: Category::Movies,
                id: 1,
                title: "Dune".into(),
                original_title: "Dune".into(),
                release_date: Some("2021-09-15".into()),
                overview: String::new(),
            }),
            None,
        );
        assert_eq!(title(&v), "Dune");
    }
}
