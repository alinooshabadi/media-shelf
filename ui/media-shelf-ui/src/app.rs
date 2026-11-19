//! Application state, background work, and the CUI frame callback.
//!
//! `App` holds everything the UI needs. The frame callback (a plain C function
//! pointer, so no `Box<dyn>` / `Rc` inside the tree) reads the state through
//! `&mut *user`, draws one CUI frame, and processes background results.

use crate::{ffi, library, series};
use media_shelf::storage::{LibraryDb, database_path};
use media_shelf::{Category, ScanReport, Source, Video};
use std::{
    collections::HashSet,
    ffi::c_void,
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
};

/// Events produced by background threads (scan, metadata reload).
pub enum Event {
    Progress(usize),
    LocalReload(Result<(Vec<Video>, usize, Vec<String>), String>),
    Done(ScanReport, Result<Vec<Video>, String>),
}

pub struct App {
    pub sources: Vec<Source>,
    pub category: Category,
    /// Folder picker text, NUL-terminated byte buffer (cap = len + 1).
    pub folder_input: Vec<u8>,
    /// Search text, NUL-terminated byte buffer (cap = len + 1).
    pub filter: Vec<u8>,
    pub videos: Vec<Video>,
    pub series_tree: series::SeriesTree,
    pub warnings: Vec<String>,
    pub error: Option<String>,
    pub status: String,
    pub receiver: Option<mpsc::Receiver<Event>>,
    pub cancel: Arc<AtomicBool>,
    pub database: Option<LibraryDb>,
    pub selected: Option<PathBuf>,
    pub selected_season: Option<(PathBuf, u32)>,
    pub selection: HashSet<PathBuf>,
    pub detail_tab: i32,
    pub sources_open: bool,
    pub settings_open: bool,
    /// "Show missing seasons and episodes" (persisted in the database).
    pub show_missing_episodes: bool,
    pub filter_mode: i32,
    pub sort: library::SortColumn,
    pub sort_descending: bool,
    /// Theme override; `None` follows the system.
    pub theme_dark: Option<bool>,
    /// Library/detail split position (session-only).
    pub split_ratio: f32,
    /// GPU poster textures, keyed by file path (loaded once, drawn many times).
    pub poster_cache: std::collections::HashMap<std::path::PathBuf, (ffi::cui_texture, i32, i32)>,
    /// Show folders whose tree node is expanded (CUI owns the `bool` state).
    pub show_open: std::collections::HashMap<std::path::PathBuf, bool>,
    /// (folder, season) pairs whose season tree node is expanded.
    pub season_open: std::collections::HashMap<(std::path::PathBuf, u32), bool>,
    /// Transient one-shot toast shown on the next frame (e.g. pending features).
    pub toast_msg: Option<String>,
    /// Context-menu action chosen this frame, consumed by the series tree.
    pub pending_show_action: Option<crate::library::ShowAction>,
}

impl App {
    pub fn new() -> Self {
        let loaded = (|| -> media_shelf::storage::DbResult<_> {
            let path = database_path();
            let mut db = LibraryDb::open(&path)?;
            db.migrate_folders(&path.with_file_name("folders.json"))?;
            let sources = db.sources()?;
            let show_missing = db.show_missing_episodes()?;
            let warnings = db.import_local_nfos()?;
            let videos = db.videos()?;
            Ok((db, sources, show_missing, videos, warnings))
        })();
        let (database, sources, show_missing_episodes, videos, warnings, error) = match loaded {
            Ok((db, sources, show_missing, videos, warnings)) => {
                (Some(db), sources, show_missing, videos, warnings, None)
            }
            Err(e) => (
                None,
                vec![],
                false,
                vec![],
                vec![],
                Some(format!(
                    "Could not open library database: {e}. Restart after fixing the problem. Database: {}",
                    database_path().display()
                )),
            ),
        };
        let status = if sources.is_empty() {
            "Add your first folder to get started".into()
        } else {
            format!(
                "Loaded {} saved items · Rescan folders to check for changes",
                videos.len()
            )
        };
        // Start on the category with the most items (override:
        // MEDIA_SHELF_CATEGORY=movies|series). The selection follows so the
        // detail pane is never empty on launch.
        let movies = videos.iter().filter(|v| v.category == Category::Movies).count();
        let series = videos.iter().filter(|v| v.category == Category::Series).count();
        let category = match std::env::var("MEDIA_SHELF_CATEGORY").as_deref() {
            Ok("movies") | Ok("") => Category::Movies,
            Ok("series") => Category::Series,
            _ if series > movies => Category::Series,
            _ => Category::Movies,
        };
        let selected = videos
            .iter()
            .find(|v| v.category == category)
            .map(|v| v.path.clone());
        let selection = selected.iter().cloned().collect();
        Self {
            sources,
            category,
            folder_input: Vec::new(),
            filter: Vec::new(),
            videos,
            series_tree: Default::default(),
            warnings,
            error,
            status,
            receiver: None,
            cancel: Arc::new(AtomicBool::new(false)),
            database,
            selected,
            selected_season: None,
            selection,
            detail_tab: 0,
            sources_open: false,
            settings_open: false,
            show_missing_episodes,
            filter_mode: 0,
            split_ratio: 0.34,
            poster_cache: Default::default(),
            show_open: Default::default(),
            season_open: Default::default(),
            toast_msg: None,
            pending_show_action: None,
            sort: library::SortColumn::Title,
            sort_descending: false,
            theme_dark: std::env::var("MEDIA_SHELF_THEME").ok().and_then(|v| match v.as_str() {
                "dark" => Some(true),
                "light" => Some(false),
                _ => None,
            }),
        }
    }

    fn save_sources(&mut self, sources: Vec<Source>) -> bool {
        let result = self
            .database
            .as_mut()
            .ok_or_else(|| "Database unavailable".to_string())
            .and_then(|db| db.set_sources(&sources).map_err(|e| e.to_string()));
        if let Err(e) = result {
            self.error = Some(format!("Could not save folders: {e}"));
            return false;
        }
        self.sources = sources;
        if let Some(db) = &self.database {
            match db.videos() {
                Ok(videos) => self.videos = videos,
                Err(e) => self.error = Some(format!("Could not reload cached files: {e}")),
            }
        }
        true
    }

    pub fn add_folder(&mut self, path: PathBuf) {
        if !path.is_dir() {
            self.error = Some(format!("Choose an existing folder: {}", path.display()));
            return;
        }
        let path = match path.canonicalize() {
            Ok(path) => path,
            Err(e) => {
                self.error = Some(format!("Could not open folder: {e}"));
                return;
            }
        };
        let source = Source {
            category: self.category,
            path,
        };
        if self.sources.contains(&source) {
            self.error = Some("That folder is already in this category.".into());
            return;
        }
        let mut sources = self.sources.clone();
        sources.push(source);
        if !self.save_sources(sources) {
            return;
        }
        self.folder_input.clear();
        self.error = None;
        self.start_scan();
    }

    pub fn reload_local_metadata(&mut self) {
        if self.database.is_none() || self.receiver.is_some() {
            return;
        }
        let paths: Vec<_> = library::rows(self)
            .into_iter()
            .map(|i| self.videos[i].path.clone())
            .collect();
        if paths.is_empty() {
            return;
        }
        let (sender, receiver) = mpsc::channel();
        self.receiver = Some(receiver);
        self.status = "Reading local metadata…".into();
        self.error = None;
        self.warnings.clear();
        std::thread::spawn(move || {
            let result = (|| -> media_shelf::storage::DbResult<_> {
                let mut db = LibraryDb::open(&database_path())?;
                let (count, warnings) = db.reload_local_nfos(&paths)?;
                Ok((db.videos()?, count, warnings))
            })()
            .map_err(|e| e.to_string());
            let _ = sender.send(Event::LocalReload(result));
        });
    }

    pub fn start_scan(&mut self) {
        if self.receiver.is_some() || self.database.is_none() {
            return;
        }
        self.series_tree = Default::default();
        self.warnings.clear();
        self.selected = None;
        self.selected_season = None;
        self.selection.clear();
        let sources = self.sources.clone();
        let cancel = Arc::new(AtomicBool::new(false));
        self.cancel = cancel.clone();
        let (sender, receiver) = mpsc::channel();
        self.receiver = Some(receiver);
        self.status = "Scanning folders…".into();
        std::thread::spawn(move || {
            let mut report = media_shelf::index(&sources, &cancel, |n| {
                let _ = sender.send(Event::Progress(n));
            });
            let saved = (|| -> media_shelf::storage::DbResult<_> {
                let mut db = LibraryDb::open(&database_path())?;
                db.apply_scan(&sources, &report)?;
                if !report.cancelled {
                    report.warnings.extend(db.import_local_nfos()?);
                }
                db.videos()
            })()
            .map_err(|e| e.to_string());
            let _ = sender.send(Event::Done(report, saved));
        });
    }

    pub fn cancel_scan(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
    }

    /// The single selected video, if it is still in the library.
    pub fn selected_video(&self) -> Option<&Video> {
        self.selected.as_ref().and_then(|p| {
            self.videos
                .iter()
                .find(|v| v.path == *p)
        })
    }

    pub fn items_locked(&self) -> bool {
        self.receiver.is_some()
    }

    /// Select one item. `additive` (Ctrl/Cmd-click) toggles it inside the
    /// multi-selection; a plain click replaces it.
    pub fn select(&mut self, path: PathBuf, additive: bool) {
        self.selected_season = None;
        if let Some(video) = self
            .videos
            .iter_mut()
            .find(|v| v.category == Category::Series && v.path == path)
        {
            media_shelf::artwork::attach_one(video, &self.sources);
        }
        if !additive {
            self.selection.clear();
        }
        if additive && self.selection.contains(&path) {
            self.selection.remove(&path);
        } else {
            self.selection.insert(path.clone());
        }
        self.selected = Some(path);
        self.detail_tab = 0;
    }

    /// Select a season of a show folder (from a season header click).
    pub fn select_season(&mut self, folder: PathBuf, season: u32) {
        self.selection.clear();
        self.selection.insert(folder.clone());
        self.selected = Some(folder.clone());
        self.selected_season = Some((folder, season));
    }

    /// Switch category, resetting per-category view state.
    pub fn select_category(&mut self, category: Category) {
        self.category = category;
        self.sort = library::SortColumn::Title;
        self.sort_descending = false;
        self.selection.clear();
        self.selected = None;
        self.selected_season = None;
        self.detail_tab = 0;
    }

    /// TMDB catalog lookup for a matched show: (show id, token, language).
    pub fn episode_lookup(&self, show: &Video) -> Option<(i64, String, String)> {
        show.tmdb.as_ref().map(|m| (m.id, String::new(), "en".to_string()))
    }

    /// Drop a loaded show and request it again (Refresh episodes).
    pub fn reload_show(&mut self, folder: &std::path::Path) {
        self.series_tree.shows.remove(folder);
        let catalog = self
            .selected_video()
            .and_then(|v| self.episode_lookup(v));
        self.series_tree
            .request(folder, &self.sources, catalog, self.show_missing_episodes);
    }

    /// Persist the "show missing episodes" preference (Settings dialog).
    pub fn set_show_missing_episodes(&mut self, enabled: bool) {
        if let Some(db) = self.database.as_mut() {
            if let Err(e) = db.set_show_missing_episodes(enabled) {
                self.error = Some(format!("Could not save setting: {e}"));
                return;
            }
        }
        self.show_missing_episodes = enabled;
        if let Some(selected) = self.selected.clone() {
            self.reload_show(&selected);
        }
    }
}

impl Drop for App {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
    }
}

/// Drain background events. Call once per frame.
pub fn poll(ui: crate::ui::Ui, app: &mut App) {
    app.series_tree.poll(ui);
    let mut done = false;
    if let Some(receiver) = &app.receiver {
        loop {
            match receiver.try_recv() {
                Ok(Event::LocalReload(result)) => match result {
                    Ok((videos, count, warnings)) => {
                        app.videos = videos;
                        app.series_tree = Default::default();
                        app.status =
                            format!("Reloaded local metadata for {count} item(s) · {} warnings", warnings.len());
                        app.warnings = warnings;
                    }
                    Err(e) => app.error = Some(format!("Could not reload local metadata: {e}")),
                },
                Ok(Event::Progress(n)) => {
                    app.status = format!("Scanning folders… {n} movies / show folders found");
                }
                Ok(Event::Done(report, saved)) => {
                    app.status = if report.cancelled {
                        "Scan cancelled · Saved library kept".into()
                    } else if !report.warnings.is_empty() {
                        format!(
                            "Found {} movies / show folders · {} warnings · Cached items retained",
                            report.videos.len(),
                            report.warnings.len()
                        )
                    } else {
                        format!(
                            "Scan complete · {} movies / show folders · {} warnings",
                            report.videos.len(),
                            report.warnings.len()
                        )
                    };
                    match saved {
                        Ok(videos) => {
                            app.videos = videos;
                        }
                        Err(e) => {
                            app.status = "Scan could not be saved · Showing previous library".into();
                            app.error = Some(format!("Database update failed: {e}"));
                        }
                    }
                    app.warnings = report.warnings;
                    done = true;
                }
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => {
                    app.error = Some("The scanner stopped unexpectedly. Try scanning again.".into());
                    done = true;
                }
            }
            if done {
                break;
            }
        }
    }
    if done {
        app.receiver = None;
    }
}

/// The CUI frame callback. Must be a plain C function pointer: no `Box<dyn>`,
/// no `Rc`, no `Thread`-local state — the app is passed through `user`.
pub extern "C" fn frame(ui: *mut crate::ffi::cui_ctx, user: *mut c_void) {
    // `user` is a `Box<App>` created in `run`; it lives until `cui_app_run`
    // returns, and exactly one frame callback runs at a time per app.
    let app = unsafe { &mut *(user as *mut App) };
    let ui = ui as crate::ui::Ui;
    poll(ui, app);
    render(ui, app);
}

fn render(ui: crate::ui::Ui, app: &mut App) {
    use crate::ui as u;

    if let Some(dark) = app.theme_dark {
        u::set_theme(ui, dark);
    }

    let scanning = app.receiver.is_some();
    let locked = scanning;

    // Root column: toolbar / split / status bar.
    let mut root = unsafe { u::opts::<ffi::cui_box>() };
    root.dir = ffi::cui_dir_CUI_COLUMN;
    root.width = u::fill();
    root.height = u::fill();
    root.align = ffi::cui_align_CUI_ALIGN_STRETCH;
    root.bg = u::bg();
    u::box_(ui, &root, |ui| {
        toolbar(ui, app, locked);
        u::separator(ui);
        let mut split_opts = unsafe { u::opts::<ffi::cui_split_opts>() };
        split_opts.min_first = 380.0;
        split_opts.min_second = 380.0;
        let mut ratio = app.split_ratio;
        let _split_guard = u::split_begin(ui, "main_split", &mut ratio, &split_opts);
        library::library_pane(ui, app);
        u::split_next(ui);
        detail_view(ui, app);
        app.split_ratio = ratio;
        status_bar(ui, app);
    });

    sources_dialog(ui, app, locked);
    settings_dialog(ui, app);

    // Flash scan errors as toasts.
    if let Some(error) = app.error.clone() {
        let mut topts = unsafe { u::opts::<ffi::cui_toast_opts>() };
        topts.tone = ffi::cui_tone_CUI_TONE_DANGER;
        topts.description = u::cstr(&error);
        u::toast(ui, "Media Shelf", &topts);
        app.error = None;
    }
    // One-shot informational toasts (pending features, confirmations).
    if let Some(msg) = app.toast_msg.take() {
        let mut topts = unsafe { u::opts::<ffi::cui_toast_opts>() };
        topts.tone = ffi::cui_tone_CUI_TONE_INFO;
        topts.description = u::cstr(&msg);
        u::toast(ui, "Media Shelf", &topts);
    }
}

fn toolbar(ui: crate::ui::Ui, app: &mut App, locked: bool) {
    use crate::ui as u;

    let mut bar = unsafe { u::opts::<ffi::cui_box>() };
    bar.dir = ffi::cui_dir_CUI_ROW;
    bar.width = u::fill();
    bar.height = u::fit();
    bar.align = ffi::cui_align_CUI_ALIGN_CENTER;
    bar.gap = 12.0;
    bar.pad_x = 16.0;
    bar.pad_y = 10.0;
    bar.bg = u::surface();
    u::box_(ui, &bar, |ui| {
        let mut topts = unsafe { u::opts::<ffi::cui_text_opts>() };
        topts.size = 20.0;
        topts.font = ffi::CUI_FONT_MEDIUM as i32;
        u::text(ui, "Media Shelf", &topts);
        u::spacer(ui);

        if u::button(
            ui,
            "Update sources",
            &opts_button(ffi::cui_button_variant_CUI_BUTTON_SECONDARY, locked),
        ) && !locked {
            app.start_scan();
        }
        if u::button(
            ui,
            "Search & scrape",
            &opts_button(ffi::cui_button_variant_CUI_BUTTON_PRIMARY, locked || app.selected.is_none()),
        ) && !locked && app.selected.is_some() {
            app.toast_msg = Some("TMDB matching is on the way — this opens the scrape dialog.".into());
        }
        u::menu(ui, "More", &menu_opts_default(), |ui| {
            if u::menu_item(ui, "Search & scrape selected", &menu_item_opts(false, app.selected.is_none())) {
                app.toast_msg = Some("TMDB matching is on the way — this opens the scrape dialog.".into());
            }
            u::menu_separator(ui);
            if u::menu_item(ui, "Data sources…", &menu_item_opts(false, false)) {
                app.sources_open = true;
            }
            if u::menu_item(ui, "Settings…", &menu_item_opts(false, false)) {
                app.settings_open = true;
            }
        });
    });
}

fn detail_view(ui: crate::ui::Ui, app: &mut App) {
    use crate::ui as u;

    let mut pane = unsafe { u::opts::<ffi::cui_box>() };
    pane.dir = ffi::cui_dir_CUI_COLUMN;
    pane.width = u::fill();
    pane.height = u::fill();
    pane.align = ffi::cui_align_CUI_ALIGN_STRETCH;
    pane.bg = u::bg();
    u::box_(ui, &pane, |ui| {
        let Some(video) = app.selected_video().cloned() else {
            u::spacer(ui);
            let mut t = unsafe { u::opts::<ffi::cui_text_opts>() };
            t.color = u::text_muted();
            u::text(ui, "Select a movie or TV show to see its details.", &t);
            u::spacer(ui);
            return;
        };

        u::heading(ui, &library::title(&video), 1);

        let tabs = ["Overview", "Cast & crew", "Media files", "Artwork", "Trailer"];
        if u::tabs(ui, "detail_tabs", &tabs, &mut app.detail_tab, &tabs_opts()) {
            u::request_redraw(ui);
        }

        let mut scroll = unsafe { u::opts::<ffi::cui_scroll_opts>() };
        scroll.width = u::fill();
        scroll.height = u::fill();
        scroll.id = u::cstr("detail_scroll");
        scroll.pad = 16.0;
        scroll.gap = 12.0;
        u::scroll(ui, &scroll, |ui| {
            let data = video.metadata.as_ref();
            match app.detail_tab {
                0 => overview_tab(ui, app, &video, data),
                1 => cast_tab(ui, data),
                2 => media_files_tab(ui, app, &video),
                3 => artwork_tab(ui, app, &video),
                4 => trailer_tab(ui, &video),
                _ => {}
            }
        });
    });
}

fn overview_tab(ui: crate::ui::Ui, _app: &mut App, _video: &Video, data: Option<&media_shelf::metadata::Metadata>) {
    use crate::ui as u;
    if let Some(m) = data {
        if !m.identity.overview.is_empty() {
            let mut o = unsafe { u::opts::<ffi::cui_text_opts>() };
            o.width = u::fill();
            u::text(ui, &m.identity.overview, &o);
        }
        u::gap(ui, 8.0);
        for (label, value) in [
            ("Year", m.year.map(|y| y.to_string()).unwrap_or_default()),
            ("Rating", m.rating.map(|r| format!("{r:.1}")).unwrap_or_default()),
            ("Runtime", m.runtime.map(|r| format!("{r} min")).unwrap_or_default()),
            ("IMDb", m.imdb_id.clone().unwrap_or_default()),
        ] {
            if value.is_empty() { continue; }
            u::row(ui, &opts_row(), |ui| {
                let mut o = unsafe { u::opts::<ffi::cui_text_opts>() };
                o.color = u::text_muted();
                o.width = u::px(90.0);
                u::text(ui, label, &o);
                let v = unsafe { u::opts::<ffi::cui_text_opts>() };
                u::text(ui, &value, &v);
            });
        }
        if !m.genres.is_empty() {
            u::gap(ui, 8.0);
            let joined = m.genres.join(", ");
            let mut o = unsafe { u::opts::<ffi::cui_text_opts>() };
            o.width = u::fill();
            u::text(ui, &joined, &o);
        }
    } else {
        let mut t = unsafe { u::opts::<ffi::cui_text_opts>() };
        t.color = u::text_muted();
        u::text(ui, "No metadata yet. Use Search & scrape to fetch details.", &t);
    }
}

fn cast_tab(ui: crate::ui::Ui, data: Option<&media_shelf::metadata::Metadata>) {
    use crate::ui as u;
    let Some(m) = data else {
        let mut t = unsafe { u::opts::<ffi::cui_text_opts>() };
        t.color = u::text_muted();
        u::text(ui, "Search & scrape to add cast and crew.", &t);
        return;
    };
    for p in m.crew.iter().chain(m.cast.iter()) {
        u::row(ui, &opts_row(), |ui| {
            let o = unsafe { u::opts::<ffi::cui_text_opts>() };
            u::text(ui, &p.name, &o);
            let mut r = unsafe { u::opts::<ffi::cui_text_opts>() };
            r.color = u::text_muted();
            u::text(ui, &p.role, &r);
        });
    }
    if m.crew.is_empty() && m.cast.is_empty() {
        let mut t = unsafe { u::opts::<ffi::cui_text_opts>() };
        t.color = u::text_muted();
        u::text(ui, "No cast or crew recorded.", &t);
    }
}

fn media_files_tab(ui: crate::ui::Ui, app: &mut App, video: &Video) {
    use crate::ui as u;
    if video.is_folder {
        let mut t = unsafe { u::opts::<ffi::cui_text_opts>() };
        t.color = u::text_muted();
        t.width = u::fill();
        u::text(ui, "Expand this show in the library to browse seasons and episodes.", &t);
        if u::link(ui, "Open containing folder") {
            if let Some(e) = library::open_folder(&video.path) {
                app.error = Some(e);
            }
        }
        return;
    }
    let mut o = unsafe { u::opts::<ffi::cui_text_opts>() };
    o.width = u::fill();
    u::text(ui, &video.path.file_name().unwrap_or_default().to_string_lossy(), &o);
    u::gap(ui, 8.0);
    let mut o = unsafe { u::opts::<ffi::cui_text_opts>() };
    o.color = u::text_muted();
    o.width = u::fill();
    u::text(ui, &format!("Size: {}", size_label(video.bytes)), &o);
    if let Some(folder) = video.path.parent() {
        u::gap(ui, 8.0);
        if u::link(ui, "Open containing folder") {
            if let Some(e) = library::open_folder(folder) {
                app.error = Some(e);
            }
        }
    }
}

fn artwork_tab(ui: crate::ui::Ui, app: &mut App, video: &Video) {
    use crate::ui as u;
    let poster = video.artwork.poster.clone().or_else(|| {
        video.metadata.as_ref().and_then(|m| m.poster_file.clone())
    });
    match poster {
        Some(path) if path.exists() => {
            let loaded = app.poster_cache.entry(path.clone()).or_insert_with(|| {
                u::image_load(ui, &path.display().to_string())
            });
            let (tex, w, h) = *loaded;
            if tex != 0 && w > 0 && h > 0 {
                let mut io = unsafe { u::opts::<ffi::cui_image_opts>() };
                io.width = u::px((w as f32 / h as f32 * 240.0).min(480.0));
                io.height = u::px(240.0);
                io.radius = 8.0;
                u::image(ui, tex, &io);
            } else {
                let mut t = unsafe { u::opts::<ffi::cui_text_opts>() };
                t.color = u::text_muted();
                u::text(ui, "Poster could not be decoded (PNG/JPEG only).", &t);
            }
        }
        _ => {
            let mut t = unsafe { u::opts::<ffi::cui_text_opts>() };
            t.color = u::text_muted();
            u::text(ui, "No poster available. Scrape to fetch artwork.", &t);
        }
    }
}

fn trailer_tab(ui: crate::ui::Ui, video: &Video) {
    use crate::ui as u;
    let trailer = video.metadata.as_ref().and_then(|m| m.trailer.clone());
    match trailer {
        Some(url) if !url.is_empty() => {
            if u::link(ui, "Open trailer") {
                u::open_url(&url);
            }
        }
        _ => {
            let mut t = unsafe { u::opts::<ffi::cui_text_opts>() };
            t.color = u::text_muted();
            u::text(ui, "No trailer recorded.", &t);
        }
    }
}

fn status_bar(ui: crate::ui::Ui, app: &mut App) {
    use crate::ui as u;
    let opts = unsafe { u::opts::<ffi::cui_status_bar_opts>() };
    u::status_bar(ui, "status", &opts, |ui| {
        let text_opts = unsafe { u::opts::<ffi::cui_status_item_opts>() };
        if app.receiver.is_some() {
            u::status_bar_text(ui, &app.status, &text_opts);
        } else {
            u::status_bar_text(ui, &app.status, &text_opts);
        }
        u::status_bar_spacer(ui);
        if let Some(error) = &app.error {
            let mut eo = unsafe { u::opts::<ffi::cui_status_item_opts>() };
            eo.tone = ffi::cui_tone_CUI_TONE_DANGER;
            eo.tooltip = u::cstr(error);
            u::status_bar_item(ui, "⚠", &eo);
        }
    });
}

fn sources_dialog(ui: crate::ui::Ui, app: &mut App, locked: bool) {
    use crate::ui as u;
    if !app.sources_open { return; }
    let mut open = true;
    let mut dopts = unsafe { u::opts::<ffi::cui_dialog_opts>() };
    dopts.width = 650.0;
    if !u::dialog(ui, "Data sources", &mut open, &dopts, |ui| {
        sources_dialog_body(ui, app, locked);
    }) {
        app.sources_open = false;
    }
}

fn sources_dialog_body(ui: crate::ui::Ui, app: &mut App, locked: bool) {
    use crate::ui as u;
    let cats = ["Movies", "TV shows"];
    let mut cat_index = (app.category == Category::Series) as i32;
    if u::segmented(ui, "source_category", &cats, &mut cat_index, &segmented_opts()) {
        app.select_category(if cat_index == 0 { Category::Movies } else { Category::Series });
    }
    u::gap(ui, 8.0);
    let mut topts = unsafe { u::opts::<ffi::cui_text_opts>() };
    topts.color = u::text_muted();
    topts.width = u::fill();
    u::text(ui, "Add root folders. Movies scan recursively; TV shows load episodes when expanded.", &topts);
    u::gap(ui, 8.0);

    let mut add_row = unsafe { u::opts::<ffi::cui_box>() };
    add_row.dir = ffi::cui_dir_CUI_ROW;
    add_row.gap = 8.0;
    add_row.align = ffi::cui_align_CUI_ALIGN_CENTER;
    u::box_(ui, &add_row, |ui| {
        let mut io = unsafe { u::opts::<ffi::cui_input_opts>() };
        io.placeholder = u::cstr("Or paste a folder path");
        io.width = u::fill();
        let cap = app.folder_input.len() + 1;
        u::input(ui, "source_input", &mut app.folder_input, cap, &io);
        if !locked && u::button(ui, "Add", &opts_button(ffi::cui_button_variant_CUI_BUTTON_DEFAULT, false)) {
            let path = u::input_as_str(&app.folder_input).trim().trim_matches('"').to_string();
            if !path.is_empty() {
                app.add_folder(std::path::PathBuf::from(path));
                app.folder_input.clear();
            }
        }
        if !locked && u::button(ui, "Browse…", &opts_button(ffi::cui_button_variant_CUI_BUTTON_SECONDARY, false)) {
            let mut opts = unsafe { u::opts::<ffi::cui_file_dialog_opts>() };
            opts.kind = ffi::cui_file_dialog_kind_CUI_FILE_FOLDER;
            opts.title = u::cstr("Choose a media folder");
            let mut out = vec![0u8; 4096];
            if u::file_dialog(&opts, &mut out) {
                let s = u::input_as_str(&out);
                if !s.is_empty() {
                    app.add_folder(std::path::PathBuf::from(s));
                }
            }
        }
    });

    u::gap(ui, 12.0);
    let mut scroll = unsafe { u::opts::<ffi::cui_scroll_opts>() };
    scroll.width = u::fill();
    scroll.height = u::fill();
    scroll.id = u::cstr("source_list");
    scroll.gap = 8.0;
    u::scroll(ui, &scroll, |ui| {
        let mut remove: Option<usize> = None;
        for (index, s) in app.sources.iter().enumerate().filter(|(_, s)| s.category == app.category) {
            u::row(ui, &opts_row(), |ui| {
                let mut o = unsafe { u::opts::<ffi::cui_text_opts>() };
                o.width = u::fill();
                u::text(ui, &s.path.display().to_string(), &o);
                if !locked && u::button(ui, "Remove", &opts_button(ffi::cui_button_variant_CUI_BUTTON_GHOST, false)) {
                    remove = Some(index);
                }
            });
        }
        if app.sources.iter().all(|s| s.category != app.category) {
            let mut t = unsafe { u::opts::<ffi::cui_text_opts>() };
            t.color = u::text_muted();
            u::text(ui, "No folders for this category yet.", &t);
        }
        if let Some(index) = remove {
            let mut sources = app.sources.clone();
            sources.remove(index);
            if app.save_sources(sources) {
                app.start_scan();
            }
        }
    });

    u::dialog_footer(ui, |ui| {
        if u::button(ui, "Close", &opts_button(ffi::cui_button_variant_CUI_BUTTON_SECONDARY, false)) {
            app.sources_open = false;
        }
    });
}

fn settings_dialog(ui: crate::ui::Ui, app: &mut App) {
    use crate::ui as u;
    if !app.sources_open { return; }
    let mut open = true;
    let mut dopts = unsafe { u::opts::<ffi::cui_dialog_opts>() };
    dopts.width = 520.0;
    if !u::dialog(ui, "Settings", &mut open, &dopts, |ui| {
        settings_dialog_body(ui, app);
    }) {
        app.settings_open = false;
    }
}

fn settings_dialog_body(ui: crate::ui::Ui, app: &mut App) {
    use crate::ui as u;
    let mut missing = app.show_missing_episodes;
    u::checkbox(ui, "Show missing seasons and episodes", &mut missing, &check_opts());
    if missing != app.show_missing_episodes {
        app.set_show_missing_episodes(missing);
    }
    u::gap(ui, 16.0);
    u::dialog_footer(ui, |ui| {
        if u::button(ui, "Close", &opts_button(ffi::cui_button_variant_CUI_BUTTON_SECONDARY, false)) {
            app.settings_open = false;
        }
    });
}

// ---- small options builders ------------------------------------------------

pub fn opts_button(variant: ffi::cui_button_variant, disabled: bool) -> ffi::cui_button_opts {
    let mut o = unsafe { ffi_opts::<ffi::cui_button_opts>() };
    o.variant = variant;
    o.disabled = disabled;
    o
}

fn opts_row() -> ffi::cui_box {
    let mut o = unsafe { ffi_opts::<ffi::cui_box>() };
    o.dir = ffi::cui_dir_CUI_ROW;
    o.gap = 8.0;
    o.align = ffi::cui_align_CUI_ALIGN_CENTER;
    o
}

fn menu_opts_default() -> ffi::cui_menu_opts {
    unsafe { ffi_opts() }
}

pub fn menu_item_opts(danger: bool, disabled: bool) -> ffi::cui_menu_item_opts {
    let mut o = unsafe { ffi_opts::<ffi::cui_menu_item_opts>() };
    o.danger = danger;
    o.disabled = disabled;
    o
}

fn tabs_opts() -> ffi::cui_tabs_opts {
    unsafe { ffi_opts() }
}

fn segmented_opts() -> ffi::cui_segmented_opts {
    unsafe { ffi_opts() }
}

fn check_opts() -> ffi::cui_check_opts {
    unsafe { ffi_opts() }
}

unsafe fn ffi_opts<T>() -> T {
    unsafe { core::mem::zeroed() }
}

fn size_label(bytes: u64) -> String {
    if bytes >= 1 << 30 {
        format!("{:.1} GiB", bytes as f64 / (1u64 << 30) as f64)
    } else if bytes >= 1 << 20 {
        format!("{:.1} MiB", bytes as f64 / (1u64 << 20) as f64)
    } else if bytes >= 1 << 10 {
        format!("{:.1} KiB", bytes as f64 / 1024.0)
    } else {
        format!("{bytes} B")
    }
}

/// Run the application: create the window and run the CUI event loop.
pub fn run() -> Result<(), Box<dyn std::error::Error>> {
    let app = Box::new(App::new());
    let user = Box::into_raw(app) as *mut c_void;
    let code = u_run("Media Shelf", 1380, 880, 1080, 720, true, user);
    unsafe {
        drop(Box::from_raw(user as *mut App));
    }
    if code == 0 {
        Ok(())
    } else {
        Err(format!("CUI exited with code {code}").into())
    }
}

fn u_run(
    title: &str,
    width: i32,
    height: i32,
    min_width: i32,
    min_height: i32,
    follow_system_theme: bool,
    user: *mut c_void,
) -> i32 {
    crate::ui::run(title, width, height, min_width, min_height, follow_system_theme, frame, user)
}
