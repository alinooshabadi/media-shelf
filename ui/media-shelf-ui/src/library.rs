//! The movies/series library list, rendered as a virtualized CUI data grid.
//!
//! `SortColumn`, `title`, `year` and the row provider are ported from the egui
//! view; the table itself is a `cui_data_grid` (only visible rows are built).

use crate::app::App;
use crate::ffi;
use media_shelf::{Category, Video};
use std::path::Path;

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

/// Search box, filter selector, and the movies data grid.
pub fn movies_grid(ui: crate::ui::Ui, app: &mut App) {
    use crate::ui as ui2;

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
    ui2::gap(ui, 6.0);
    ui2::box_(ui, &count_bar, |ui| {
        let mut topts = unsafe { ui2::opts::<ffi::cui_text_opts>() };
        topts.color = ui2::text_muted();
        ui2::text(ui, &format!("{} items", entries.len()), &topts);
        ui2::spacer(ui);
        let mut bopts = unsafe { ui2::opts::<ffi::cui_button_opts>() };
        bopts.size = ffi::cui_control_size_CUI_SIZE_SM;
        if ui2::button(ui, "Select visible", &bopts) {
            app.selection = entries
                .iter()
                .map(|i| app.videos[*i].path.clone())
                .collect();
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
    ui2::gap(ui, 8.0);

    // The virtualized grid.
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
    // The title column fills the remaining width.
    columns[0].width = 0.0;

    let mut first: i32 = 0;
    let mut last: i32 = 0;
    if ui2::data_grid_begin(
        ui,
        "movies",
        &columns,
        n,
        &mut first,
        &mut last,
        &gopts,
    ) {
        for r in first..last {
            let e = entries[r as usize];
            let v = &app.videos[e];
            let is_selected = app.selection.contains(&v.path) || app.selected.as_ref() == Some(&v.path);
            if ui2::data_grid_row(ui, r) {
                // Row click selects (Ctrl/Shift handled by CUI's grid).
                app.selected = Some(v.path.clone());
                app.selected_season = None;
                ui2::request_redraw(ui);
            }
            let _ = is_selected;
            ui2::data_grid_cell_text(ui, &title(v));
            ui2::data_grid_cell_text(ui, &year(v));
            ui2::data_grid_cell_text(ui, &rating_label(v));
            ui2::data_grid_cell_text(ui, status_label(v));
            ui2::data_grid_cell_text(
                ui,
                v.added_date.as_deref().unwrap_or("–"),
            );
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
