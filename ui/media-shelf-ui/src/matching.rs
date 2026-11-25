//! TMDB matching & scraping: search dialog, candidate preview, scrape options,
//! token settings, and the scrape report. Ported from the egui `matching.rs`;
//! blocking API calls run on worker threads and are polled each frame.
use crate::{
    app::App,
    ffi,
    ui as u,
};
use media_shelf::{
    Category, Video,
    credentials, identification, metadata,
    storage::{LibraryDb, database_path},
    tmdb,
    tmdb::SearchPage,
    metadata::{Field, Metadata, ScrapeOptions},
};
use std::{
    collections::VecDeque,
    sync::mpsc,
};

/// TMDB search languages offered in the dialog.
const LANGUAGES: &[(&str, &str)] = &[
    ("en-US", "English"),
    ("de-DE", "Deutsch"),
    ("fr-FR", "Français"),
    ("es-ES", "Español"),
    ("it-IT", "Italiano"),
    ("pt-BR", "Português (Brasil)"),
    ("ja-JP", "日本語"),
    ("ko-KR", "한국어"),
    ("zh-CN", "中文"),
];

/// Best-guess TMDB search query for an unmatched file: the saved title, the
/// clean folder-derived identity for movies, or the file name with years,
/// episode tags, and release tags stripped.
fn suggested_query(video: &Video) -> String {
    if let Some(m) = &video.tmdb {
        return m.title.clone();
    }
    if video.category == Category::Movies {
        return identification::local_identity(video).0;
    }
    let name = (if video.is_folder {
        video.path.file_name()
    } else {
        video.path.file_stem()
    })
    .unwrap_or_default()
    .to_string_lossy()
    .replace(['.', '_'], " ");
    let stop = regex::Regex::new(
        r"(?i)\s*(?:\(?\b(?:19|20)\d{2}\b\)?|\bS\d{1,3}E\d{1,3}\b|\b\d{3,4}p\b|\b(?:bluray|web-dl|webrip|x264|x265|h264|h265)\b).*$",
    )
    .unwrap();
    let cleaned = stop.replace(&name, "").trim().to_string();
    if cleaned.is_empty() { name } else { cleaned }
}

/// One search session for a single video, including its in-flight requests.
struct SearchDialog {
    video: Video,
    query: String,
    searched_query: String,
    results: Vec<tmdb::TmdbMatch>,
    picked: Option<usize>,
    page: u32,
    total_pages: u32,
    search_rx: Option<mpsc::Receiver<Result<SearchPage, String>>>,
    detail_rx: Option<mpsc::Receiver<Result<Metadata, String>>>,
    save_rx: Option<mpsc::Receiver<Result<(usize, Vec<String>), String>>>,
    details: Option<Metadata>,
    error: Option<String>,
    searched: bool,
    whole_show: bool,
}

impl SearchDialog {
    fn new(video: Video) -> Self {
        Self {
            query: suggested_query(&video),
            whole_show: video.category == Category::Series,
            video,
            searched_query: String::new(),
            results: vec![],
            picked: None,
            page: 1,
            total_pages: 0,
            search_rx: None,
            detail_rx: None,
            save_rx: None,
            details: None,
            error: None,
            searched: false,
        }
    }

    fn search(&mut self, token: &str, language: &str, page: u32) {
        if self.search_rx.is_some() || self.save_rx.is_some() {
            return;
        }
        if token.trim().is_empty() {
            self.error = Some("Enter your TMDB Read Access Token in Settings.".into());
            return;
        }
        let query = if page == 1 {
            self.query.trim().to_string()
        } else {
            self.searched_query.clone()
        };
        self.searched_query = query.clone();
        self.query = query.clone();
        self.details = None;
        self.detail_rx = None;
        self.results.clear();
        self.picked = None;
        self.total_pages = 0;
        self.error = None;
        self.searched = true;
        let token = token.to_string();
        let language = language.to_string();
        let category = self.video.category;
        let (tx, rx) = mpsc::channel();
        self.search_rx = Some(rx);
        std::thread::spawn(move || {
            let _ = tx.send(tmdb::search_language(category, &query, &token, page, &language));
        });
    }

    fn preview(&mut self, index: usize, token: &str, language: &str) {
        let Some(row) = self.results.get(index) else {
            return;
        };
        let id = row.id;
        let category = row.category;
        self.picked = Some(index);
        self.details = None;
        self.error = None;
        let token = token.to_string();
        let language = language.to_string();
        let (tx, rx) = mpsc::channel();
        self.detail_rx = Some(rx);
        std::thread::spawn(move || {
            let _ = tx.send(metadata::details(category, id, &token, &language));
        });
    }
}

/// Matching & scraping state shared with the app.
pub struct TmdbUi {
    pub token: String,
    token_status: Option<String>,
    pub settings_open: bool,
    pub show_missing_episodes: bool,
    pub language: String,
    options: ScrapeOptions,
    dialog: Option<SearchDialog>,
    queue: VecDeque<Video>,
    total: usize,
    known_id: bool,
    pub notice: Option<String>,
    /// NUL-terminated token input buffer for the settings dialog.
    token_input: Vec<u8>,
    /// CUI-owned open flag for the search dialog (X/Esc close persists).
    search_open: bool,
    /// Whether the token buffer has been seeded from the current token yet.
    token_seeded: bool,
}

impl Default for TmdbUi {
    fn default() -> Self {
        Self {
            token: String::new(),
            token_status: None,
            settings_open: false,
            show_missing_episodes: false,
            language: "en-US".into(),
            options: ScrapeOptions::default(),
            dialog: None,
            queue: VecDeque::new(),
            total: 0,
            known_id: false,
            notice: None,
            token_input: vec![0u8],
            search_open: false,
            token_seeded: false,
        }
    }
}

impl TmdbUi {
    /// Load the saved token (falling back to `TMDB_READ_ACCESS_TOKEN`).
    pub fn new() -> Self {
        let (saved_token, token_status) =
            match credentials::load(&credentials::token_path()) {
                Ok(token) => (token, None),
                Err(_) => (
                    String::new(),
                    Some(
                        "Could not read the saved token. Enter it again and click Save token."
                            .into(),
                    ),
                ),
            };
        let mut ui = Self::default();
        ui.token = if saved_token.is_empty() {
            std::env::var("TMDB_READ_ACCESS_TOKEN").unwrap_or_default()
        } else {
            saved_token
        };
        ui.token_status = token_status;
        ui
    }

    /// True while any matching dialog or queue keeps the library locked.
    pub fn active(&self) -> bool {
        self.dialog.is_some() || !self.queue.is_empty()
    }

    pub fn open_many(&mut self, videos: Vec<Video>, known_id: bool) {
        self.queue = videos.into();
        self.total = self.queue.len();
        self.known_id = known_id;
        self.notice = None;
        self.next();
    }

    /// Advance the queue: replace the open dialog with the next video.
    fn next(&mut self) {
        self.dialog = self.queue.pop_front().map(SearchDialog::new);
        if let Some(dialog) = &mut self.dialog {
            self.search_open = true;
            if self.token.trim().is_empty() {
                self.settings_open = true;
            } else if self.known_id && let Some(m) = dialog.video.tmdb.clone() {
                dialog.results = vec![m];
                dialog.searched = true;
                dialog.preview(0, &self.token, &self.language);
            } else {
                dialog.search(&self.token, &self.language, 1);
            }
        } else {
            self.search_open = false;
        }
    }
}

/// Draw the matching dialogs: search dialog, token settings, scrape report.
/// Draw order matters: earlier = below, so the report is topmost.
pub fn render(ui: u::Ui, app: &mut App) {
    let mut reload_library = false;
    let mut advance = false;
    let mut still_polling = false;
    {
        let tui = &mut app.tmdb_ui;
        if let Some(dialog) = &mut tui.dialog {
            if let Some(rx) = &dialog.save_rx {
                still_polling = true;
                match rx.try_recv() {
                    Ok(result) => {
                        dialog.save_rx = None;
                        match result {
                            Ok((count, warnings)) => {
                                reload_library = true;
                                advance = true;
                                if !warnings.is_empty() {
                                    tui.notice = Some(format!(
                                        "Saved metadata for {count} file(s). Some outputs were kept or unavailable:\n{}",
                                        warnings.join("\n")
                                    ));
                                }
                            }
                            Err(e) => dialog.error = Some(e),
                        }
                    }
                    Err(mpsc::TryRecvError::Disconnected) => {
                        dialog.save_rx = None;
                        dialog.error =
                            Some("Scrape worker stopped. Check the library before retrying.".into());
                        reload_library = true;
                    }
                    Err(mpsc::TryRecvError::Empty) => {}
                }
            }
            if let Some(rx) = &dialog.search_rx {
                still_polling = true;
                match rx.try_recv() {
                    Ok(result) => {
                        dialog.search_rx = None;
                        match result {
                            Ok(page) => {
                                dialog.results = page.results;
                                dialog.page = page.page;
                                dialog.total_pages = page.total_pages;
                                if !dialog.results.is_empty() {
                                    dialog.preview(0, &tui.token, &tui.language);
                                }
                            }
                            Err(e) => dialog.error = Some(e),
                        }
                    }
                    Err(mpsc::TryRecvError::Disconnected) => {
                        dialog.search_rx = None;
                        dialog.error = Some("Search stopped. Try again.".into());
                    }
                    Err(mpsc::TryRecvError::Empty) => {}
                }
            }
            if let Some(rx) = &dialog.detail_rx {
                still_polling = true;
                match rx.try_recv() {
                    Ok(result) => {
                        dialog.detail_rx = None;
                        match result {
                            Ok(data) => {
                                if dialog
                                    .picked
                                    .and_then(|i| dialog.results.get(i))
                                    .is_some_and(|m| m.id == data.identity.id)
                                {
                                    dialog.details = Some(data);
                                }
                            }
                            Err(e) => dialog.error = Some(e),
                        }
                    }
                    Err(mpsc::TryRecvError::Disconnected) => {
                        dialog.detail_rx = None;
                        dialog.error = Some("Preview stopped. Select the result to retry.".into());
                    }
                    Err(mpsc::TryRecvError::Empty) => {}
                }
            }
        }
    }
    if still_polling {
        u::request_wakeup(ui, 0.08);
    }

    // The save worker finished: refresh the library from a fresh database.
    if reload_library {
        match LibraryDb::open(&database_path()) {
            Ok(db) => {
                if let Ok(videos) = db.videos() {
                    app.videos = videos;
                    app.database = Some(db);
                }
            }
            Err(e) => app.error = Some(format!("Could not reload the library: {e}")),
        }
        app.series_tree = Default::default();
        app.status = "Scrape complete · Selected metadata saved".into();
    }

    // Advance to the next queued item, or close the dialog.
    if advance {
        app.tmdb_ui.next();
    }

    // Deferred pop: while the dialog's exit spring runs `u::dialog` returns
    // false and only then is the state dropped (body already stopped running).
    if app.tmdb_ui.dialog.is_some() {
        let shown = search_dialog(ui, app);
        if !shown {
            app.tmdb_ui.dialog = None;
            app.tmdb_ui.queue.clear();
        }
    }
    if app.tmdb_ui.settings_open {
        settings_dialog(ui, app);
    }
    if app.tmdb_ui.dialog.is_none() {
        if let Some(message) = app.tmdb_ui.notice.clone() {
            notice_dialog(ui, app, &message);
        }
    }
}

// ---- small options builders ------------------------------------------------

fn label_opts() -> ffi::cui_text_opts {
    let mut o = unsafe { u::opts::<ffi::cui_text_opts>() };
    o.color = u::secondary();
    o
}

fn button_opts(variant: ffi::cui_button_variant, disabled: bool) -> ffi::cui_button_opts {
    let mut o = unsafe { u::opts::<ffi::cui_button_opts>() };
    o.variant = variant;
    o.disabled = disabled;
    o
}

fn check_opts() -> ffi::cui_check_opts {
    unsafe { u::opts::<ffi::cui_check_opts>() }
}

fn select_opts() -> ffi::cui_select_opts {
    let mut o = unsafe { u::opts::<ffi::cui_select_opts>() };
    o.width = u::fill();
    o
}

fn text_color(color: ffi::cui_color) -> ffi::cui_text_opts {
    let mut o = unsafe { u::opts::<ffi::cui_text_opts>() };
    o.color = color;
    o
}

// ---- search & scrape dialog ------------------------------------------------

/// The search-and-scrape dialog for one video. Returns whether CUI still shows
/// it (false once the exit spring finishes).
fn search_dialog(ui: u::Ui, app: &mut App) -> bool {
    use ffi::cui_button_variant::{CUI_BUTTON_PRIMARY, CUI_BUTTON_SECONDARY};
    let tui = &mut app.tmdb_ui;
    let Some(dialog) = tui.dialog.as_ref() else {
        return false;
    };
    let title = if dialog.video.category == Category::Movies {
        "Search movie"
    } else {
        "Search TV show"
    };
    let saving = dialog.save_rx.is_some();
    let searching = dialog.search_rx.is_some();
    let loading = dialog.detail_rx.is_some();
    let busy = saving || searching || loading;
    // Locking = a library scan is running. (The dialog itself does not count:
    // `items_locked()` would disable OK while its own queue is open.)
    let locked = app.receiver.is_some();

    // Keep the query buffer NUL-terminated with headroom: CUI writes at most
    // `cap - 1 - len` extra bytes and must always see a valid C string.
    let mut qbuf: Vec<u8> = dialog.query.as_bytes().to_vec();
    if qbuf.last() != Some(&0) {
        qbuf.push(0);
    }
    qbuf.reserve(48);

    let item_label = if tui.total > 1 {
        Some(format!("Item {} of {}", tui.total - tui.queue.len(), tui.total))
    } else {
        None
    };
    let description = dialog.video.path.display().to_string();
    let language_names: Vec<&str> = LANGUAGES.iter().map(|&(_, n)| n).collect();
    let lang_index = tui
        .language
        .iter()
        .zip(LANGUAGES.iter())
        .position(|(l, (code, _))| *l == *code)
        .unwrap_or(0) as i32;

    let mut dopts = unsafe { u::opts::<ffi::cui_dialog_opts>() };
    dopts.width = 1040.0;
    dopts.no_close_button = saving;
    dopts.description = u::cstr(&description);

    let shown = u::dialog(ui, title, &mut tui.search_open, &dopts, |ui| {
        if !tui.search_open || tui.dialog.is_none() {
            return; // exit-spring frames: state already advanced/cleared
        }
        let vh = u::viewport_size(ui).y;
        let body_h = (vh - 224.0).max(140.0);

        let mut scroll = unsafe { u::opts::<ffi::cui_scroll_opts>() };
        scroll.width = u::fill();
        scroll.height = u::px(body_h);
        scroll.id = u::cstr("scrape_body");
        scroll.gap = 10.0;
        u::scroll(ui, &scroll, |ui| {
            // Path + item counter.
            u::text(ui, &description, &label_opts());
            if let Some(item) = &item_label {
                u::text(ui, item, &text_color(u::text_muted()));
            }
            u::separator(ui);

            // Row 1: Scraper / query / Search.
            let mut row1 = unsafe { u::opts::<ffi::cui_box>() };
            row1.dir = ffi::cui_dir_CUI_ROW;
            row1.gap = 10.0;
            row1.align = ffi::cui_align_CUI_ALIGN_CENTER;
            u::row(ui, &row1, |ui| {
                u::text(ui, "Scraper", &label_opts());
                let mut scraper = 0i32;
                let mut so = select_opts();
                so.width = u::px(170.0);
                u::select(ui, "scraper", &["themoviedb.org"], &mut scraper, &so);
                let mut qo = unsafe { u::opts::<ffi::cui_input_opts>() };
                qo.placeholder = u::cstr("Title to search for");
                qo.width = u::fill();
                let flags = u::search(ui, "tmdb_query", &mut qbuf, qbuf.len() + 1 + 32, &qo);
                if (flags & ffi::CUI_CHANGED) != 0 {
                    app.tmdb_ui.dialog.as_mut().unwrap().query = u::input_as_str(&qbuf).to_string();
                }
                if !busy && !locked {
                    let wants_search = u::button(ui, "Search", &button_opts(CUI_BUTTON_PRIMARY, false))
                        || (flags & ffi::CUI_SUBMITTED) != 0;
                    if wants_search {
                        app.tmdb_ui.dialog.as_mut().unwrap().search(
                            &app.tmdb_ui.token,
                            &app.tmdb_ui.language,
                            1,
                        );
                    }
                }
            });

            // Row 2: Language / API settings.
            let mut row2 = unsafe { u::opts::<ffi::cui_box>() };
            row2.dir = ffi::cui_dir_CUI_ROW;
            row2.gap = 10.0;
            row2.align = ffi::cui_align_CUI_ALIGN_CENTER;
            u::row(ui, &row2, |ui| {
                u::text(ui, "Language", &label_opts());
                let mut lang = lang_index;
                let mut so = select_opts();
                so.width = u::px(170.0);
                if u::select(ui, "tmdb_lang", &language_names, &mut lang, &so) && lang != lang_index {
                    app.tmdb_ui.language = LANGUAGES[lang as usize].0.to_string();
                    app.tmdb_ui
                        .dialog
                        .as_mut()
                        .unwrap()
                        .search(&app.tmdb_ui.token, &app.tmdb_ui.language, 1);
                }
                if !busy
                    && u::button(ui, "API settings", &button_opts(CUI_BUTTON_SECONDARY, false))
                {
                    app.tmdb_ui.settings_open = true;
                }
            });
            u::separator(ui);

            // Two panes: candidate list | detail preview.
            let mut panes = unsafe { u::opts::<ffi::cui_box>() };
            panes.dir = ffi::cui_dir_CUI_ROW;
            panes.gap = 12.0;
            u::box_(ui, &panes, |ui| {
                let mut left = unsafe { u::opts::<ffi::cui_box>() };
                left.dir = ffi::cui_dir_CUI_COLUMN;
                left.width = u::px(265.0);
                left.gap = 8.0;
                u::column(ui, &left, |ui| {
                    u::text(ui, "Search result", &text_color(u::fg()));
                    let mut cs = unsafe { u::opts::<ffi::cui_scroll_opts>() };
                    cs.width = u::fill();
                    cs.height = u::px(280.0);
                    cs.id = u::cstr("candidate_list");
                    u::scroll(ui, &cs, |ui| {
                        for (index, result) in app.tmdb_ui.dialog.as_ref().unwrap().results.iter().enumerate() {
                            let mut lo = unsafe { u::opts::<ffi::cui_list_item_opts>() };
                            lo.subtitle = u::cstr(result.year());
                            let _ = index; // id pushed below
                            u::push_id_int(ui, index as i32);
                            let clicked = u::list_item(ui, &result.title, &lo);
                            u::pop_id(ui);
                            if clicked && !saving {
                                app.tmdb_ui.dialog.as_mut().unwrap().preview(
                                    index,
                                    &app.tmdb_ui.token,
                                    &app.tmdb_ui.language,
                                );
                            }
                        }
                        let d = app.tmdb_ui.dialog.as_ref().unwrap();
                        if d.searched && d.results.is_empty() && d.search_rx.is_none() {
                            u::text(ui, "No results. Edit the title and search again.", &text_color(u::text_muted()));
                        }
                    });
                    let page = app.tmdb_ui.dialog.as_ref().unwrap().page;
                    let total_pages = app.tmdb_ui.dialog.as_ref().unwrap().total_pages;
                    let mut prow = unsafe { u::opts::<ffi::cui_box>() };
                    prow.dir = ffi::cui_dir_CUI_ROW;
                    prow.gap = 8.0;
                    u::row(ui, &prow, |ui| {
                        if u::button(ui, "Previous", &button_opts(ffi::cui_button_variant_CUI_BUTTON_DEFAULT, page <= 1 || busy))
                            && page > 1
                        {
                            let page = page - 1;
                            let d = app.tmdb_ui.dialog.as_mut().unwrap();
                            d.search(&app.tmdb_ui.token, &app.tmdb_ui.language, page);
                        }
                        u::text(ui, &format!("{page} / {}", total_pages.max(1)), &text_color(u::text_muted()));
                        if u::button(ui, "Next", &button_opts(ffi::cui_button_variant_CUI_BUTTON_DEFAULT, page >= total_pages || busy))
                            && page < total_pages
                        {
                            let page = page + 1;
                            let d = app.tmdb_ui.dialog.as_mut().unwrap();
                            d.search(&app.tmdb_ui.token, &app.tmdb_ui.language, page);
                        }
                    });
                });

                let mut right = unsafe { u::opts::<ffi::cui_box>() };
                right.dir = ffi::cui_dir_CUI_COLUMN;
                right.width = u::fill();
                right.gap = 8.0;
                u::column(ui, &right, |ui| {
                    if searching || loading {
                        let mut spin = unsafe { u::opts::<ffi::cui_box>() };
                        spin.dir = ffi::cui_dir_CUI_ROW;
                        spin.gap = 10.0;
                        spin.align = ffi::cui_align_CUI_ALIGN_CENTER;
                        u::row(ui, &spin, |ui| {
                            u::spinner(ui, 16.0);
                            u::text(
                                ui,
                                if searching { "Searching…" } else { "Loading candidate details…" },
                                &text_color(u::text_muted()),
                            );
                        });
                    }
                    let data = app.tmdb_ui.dialog.as_ref().and_then(|d| d.details.clone());
                    if let Some(data) = data {
                        let poster = data.poster_file.clone();
                        let mut topt = unsafe { u::opts::<ffi::cui_box>() };
                        topt.dir = ffi::cui_dir_CUI_ROW;
                        topt.gap = 12.0;
                        u::box_(ui, &topt, |ui| {
                            match poster {
                                Some(path) if path.exists() => {
                                    let loaded =
                                        app.poster_cache.entry(path.clone()).or_insert_with(|| {
                                            u::image_load(ui, &path.display().to_string())
                                        });
                                    let (tex, w, h) = *loaded;
                                    if tex != 0 && w > 0 && h > 0 {
                                        let mut io = unsafe { u::opts::<ffi::cui_image_opts>() };
                                        io.width = u::px(145.0);
                                        io.height = u::px(210.0);
                                        io.radius = 8.0;
                                        u::image(ui, tex, &io);
                                    } else {
                                        u::text(ui, "No poster", &text_color(u::text_muted()));
                                    }
                                }
                                _ => {
                                    u::text(ui, "No poster", &text_color(u::text_muted()));
                                }
                            }
                            let mut info = unsafe { u::opts::<ffi::cui_box>() };
                            info.dir = ffi::cui_dir_CUI_COLUMN;
                            info.width = u::fill();
                            info.gap = 6.0;
                            u::column(ui, &info, |ui| {
                                u::heading(
                                    ui,
                                    &format!(
                                        "{} ({})",
                                        data.identity.title,
                                        data.year.map(|y| y.to_string()).unwrap_or_else(|| "—".into())
                                    ),
                                    1,
                                );
                                u::text(ui, &data.identity.original_title, &label_opts());
                                if let Some(tagline) = &data.tagline {
                                    u::text(ui, tagline, &text_color(u::text_muted()));
                                }
                                let mut ps = unsafe { u::opts::<ffi::cui_scroll_opts>() };
                                ps.width = u::fill();
                                ps.height = u::px(155.0);
                                ps.id = u::cstr("candidate_plot");
                                u::scroll(ui, &ps, |ui| {
                                    u::text(ui, &data.identity.overview, &text_color(u::fg()));
                                });
                            });
                        });
                        u::separator(ui);

                        // People: key crew first, then cast (old egui order).
                        let mut people: Vec<&media_shelf::metadata::Person> = data
                            .crew
                            .iter()
                            .filter(|p| {
                                matches!(
                                    p.role.as_str(),
                                    "Director" | "Producer" | "Executive Producer" | "Writer" | "Screenplay" | "Creator"
                                )
                            })
                            .chain(data.cast.iter())
                            .take(100)
                            .collect();
                        if !people.is_empty() {
                            let mut sort = ffi::cui_table_sort { column: 0, descending: false };
                            let mut gopts = unsafe { u::opts::<ffi::cui_grid_opts>() };
                            gopts.width = u::fill();
                            gopts.height = u::px(140.0);
                            gopts.row_height = 26.0;
                            gopts.striped = true;
                            gopts.selected = std::ptr::null_mut();
                            gopts.sort = &mut sort;
                            gopts.hide_checkboxes = true;
                            let mut columns = [unsafe { u::opts::<ffi::cui_grid_column>() }; 2];
                            columns[0].title = u::cstr("Name");
                            columns[0].align = ffi::cui_text_align_CUI_TEXT_START;
                            columns[0].priority = -1;
                            columns[1].title = u::cstr("Role");
                            columns[1].width = 220.0;
                            columns[1].align = ffi::cui_text_align_CUI_TEXT_START;
                            let mut first: i32 = 0;
                            let mut last: i32 = 0;
                            if u::data_grid_begin(ui, "candidate_people", &columns, people.len() as i32, &mut first, &mut last, &gopts) {
                                for r in first..last {
                                    let p = &people[r as usize];
                                    if u::data_grid_row(ui, r) {
                                        let _ = p; // row click: no action
                                    }
                                    u::data_grid_cell_text(ui, &p.name);
                                    u::data_grid_cell_text(ui, &p.role);
                                }
                                u::data_grid_end(ui);
                            }
                        }
                    } else if !searching && !loading {
                        u::text(ui, "Select a candidate to preview its details.", &text_color(u::text_muted()));
                    }
                });
            });

            if let Some(error) = app.tmdb_ui.dialog.as_ref().and_then(|d| d.error.clone()) {
                u::text(ui, &error, &text_color(u::danger()));
            }
            u::separator(ui);

            // Scrape options.
            u::heading(ui, "Scrape following items", 2);
            {
                let is_series = app.tmdb_ui.dialog.as_ref().unwrap().video.category == Category::Series;
                let mut grid = unsafe { u::opts::<ffi::cui_box>() };
                grid.dir = ffi::cui_dir_CUI_GRID;
                grid.min_column_width = 170.0;
                grid.gap = 8.0;
                u::box_(ui, &grid, |ui| {
                    for field in Field::ALL {
                        if *field == Field::MovieSet && is_series {
                            continue;
                        }
                        let mut chosen = app.tmdb_ui.options.fields.contains(field);
                        if u::checkbox(ui, field.label(), &mut chosen, &check_opts()) {
                            if chosen {
                                app.tmdb_ui.options.fields.push(*field);
                            } else {
                                app.tmdb_ui.options.fields.retain(|f| f != field);
                            }
                        }
                    }
                });
                let mut brow = unsafe { u::opts::<ffi::cui_box>() };
                brow.dir = ffi::cui_dir_CUI_ROW;
                brow.gap = 8.0;
                u::row(ui, &brow, |ui| {
                    if u::button(ui, "Select all", &button_opts(ffi::cui_button_variant_CUI_BUTTON_DEFAULT, false)) {
                        let all: Vec<Field> = if is_series {
                            Field::ALL.iter().copied().filter(|f| *f != Field::MovieSet).collect()
                        } else {
                            Field::ALL.to_vec()
                        };
                        app.tmdb_ui.options.fields = all;
                    }
                    if u::button(ui, "Select none", &button_opts(ffi::cui_button_variant_CUI_BUTTON_DEFAULT, false)) {
                        app.tmdb_ui.options.fields.clear();
                    }
                    let mut om = app.tmdb_ui.options.only_missing;
                    if u::checkbox(ui, "Fill missing fields only", &mut om, &check_opts()) {
                        app.tmdb_ui.options.only_missing = om;
                    }
                    let mut wn = app.tmdb_ui.options.write_nfo;
                    if u::checkbox(ui, "Write NFO", &mut wn, &check_opts()) {
                        app.tmdb_ui.options.write_nfo = wn;
                    }
                    let mut rn = app.tmdb_ui.options.replace_nfo;
                    if u::checkbox(ui, "Replace existing NFO/artwork (backup first)", &mut rn, &check_opts()) {
                        app.tmdb_ui.options.replace_nfo = rn;
                    }
                });
                if is_series {
                    u::text(
                        ui,
                        "Applies to this TV show and its files. Individual episode titles and plots are not scraped yet.",
                        &text_color(u::text_muted()),
                    );
                }
            }
            u::separator(ui);
        });

        // Footer: the CUI footer bar is a ROW aligned end.
        let fields_empty = app.tmdb_ui.options.fields.is_empty();
        let has_details = app.tmdb_ui.dialog.as_ref().and_then(|d| d.details.as_ref()).is_some();
        let ok_enabled = !saving && !searching && !loading && !locked && has_details && !fields_empty;
        let cancel_enabled = !saving;
        let skip_enabled = tui.total > 1 && !saving;
        let scrape_started = ok_enabled
            && u::dialog_footer(ui, |ui| {
                if saving {
                    u::button(ui, "Scraping selected fields and writing outputs…", &button_opts(ffi::cui_button_variant_CUI_BUTTON_DEFAULT, true));
                }
                if u::button(ui, "OK — Scrape", &button_opts(CUI_BUTTON_PRIMARY, !ok_enabled)) {
                    let d = app.tmdb_ui.dialog.as_ref().unwrap();
                    let video = d.video.clone();
                    let data = d.details.clone().unwrap();
                    let options = app.tmdb_ui.options.clone();
                    let group = d.whole_show;
                    let (tx, rx) = mpsc::channel();
                    app.tmdb_ui.dialog.as_mut().unwrap().save_rx = Some(rx);
                    app.tmdb_ui.dialog.as_mut().unwrap().error = None;
                    std::thread::spawn(move || {
                        let result = (|| -> media_shelf::storage::DbResult<_> {
                            let mut db = LibraryDb::open(&database_path())?;
                            metadata::commit_scrape(&mut db, &video, &data, &options, group)
                        })();
                        let _ = tx.send(result.map_err(|e| e.to_string()));
                    });
                }
                if cancel_enabled && u::button(ui, "Cancel", &button_opts(ffi::cui_button_variant_CUI_BUTTON_SECONDARY, false)) {
                    app.tmdb_ui.search_open = false;
                }
                if skip_enabled && u::button(ui, "Skip", &button_opts(ffi::cui_button_variant_CUI_BUTTON_DEFAULT, false)) {
                    app.tmdb_ui.next();
                }
            })
            .is_some();
        let _ = scrape_started;
    });
    shown
}

// ---- token settings --------------------------------------------------------

/// TMDB settings & credits dialog (token, show-missing toggle, links).
fn settings_dialog(ui: u::Ui, app: &mut App) {
    let tui = &mut app.tmdb_ui;
    let mut dopts = unsafe { u::opts::<ffi::cui_dialog_opts>() };
    dopts.width = 540.0;
    let shown = u::dialog(ui, "TMDB settings & credits", &mut tui.settings_open, &dopts, |ui| {
        if !tui.settings_open {
            return; // exit-spring frames
        }
        // Seed the token buffer exactly once; afterwards CUI owns the edits.
        if !tui.token_seeded {
            tui.token_input = tui.token.as_bytes().to_vec();
            tui.token_input.push(0);
            tui.token_input.reserve(48);
            tui.token_seeded = true;
        }

        let has_db = app.database.is_some();
        let mut missing = tui.show_missing_episodes;
        if u::checkbox(ui, "Show missing seasons and episodes", &mut missing, &check_opts()) {
            if missing != tui.show_missing_episodes {
                tui.show_missing_episodes = missing;
                app.set_show_missing_episodes(missing);
            }
        }
        let _ = has_db; // the checkbox is enabled whenever the db is open
        u::text(
            ui,
            "Compare expanded, matched shows with aired episodes on TMDB. Requires a token; missing entries are display only.",
            &text_color(u::text_muted()),
        );
        u::separator(ui);

        u::text(ui, "TMDB API Read Access Token", &label_opts());
        let mut to = unsafe { u::opts::<ffi::cui_input_opts>() };
        to.password = true;
        to.width = u::fill();
        let tflags = u::input(ui, "tmdb_token", &mut tui.token_input, tui.token_input.len() + 1 + 32, &to);
        if (tflags & ffi::CUI_CHANGED) != 0 {
            tui.token = u::input_as_str(&tui.token_input).to_string();
        }
        let mut brow = unsafe { u::opts::<ffi::cui_box>() };
        brow.dir = ffi::cui_dir_CUI_ROW;
        brow.gap = 8.0;
        u::row(ui, &brow, |ui| {
            if u::button(ui, "Save token", &button_opts(ffi::cui_button_variant_CUI_BUTTON_DEFAULT, false)) {
                tui.token_status = Some(match credentials::save(&credentials::token_path(), &tui.token) {
                    Ok(()) => {
                        if tui.token.trim().is_empty() {
                            "Saved token removed.".into()
                        } else {
                            "Token saved. It will load automatically next time.".into()
                        }
                    }
                    Err(_) => "Could not save the token. You can still use it for this session.".into(),
                });
            }
            if u::button(ui, "Forget token", &button_opts(ffi::cui_button_variant_CUI_BUTTON_DEFAULT, false)) {
                tui.token_status = Some(match credentials::save(&credentials::token_path(), "") {
                    Ok(()) => {
                        tui.token.clear();
                        "Saved token removed.".into()
                    }
                    Err(_) => "Could not remove the saved token.".into(),
                });
            }
        });
        if let Some(status) = &tui.token_status {
            u::text(ui, status, &text_color(u::fg()));
        }
        u::text(
            ui,
            "Saved in your user profile. TMDB_READ_ACCESS_TOKEN is used when no saved token exists.",
            &text_color(u::text_muted()),
        );
        if u::link(ui, "TMDB account API settings") {
            u::open_url("https://www.themoviedb.org/settings/api");
        }
        u::separator(ui);
        u::text(
            ui,
            "This product uses the TMDB API but is not endorsed or certified by TMDB.",
            &text_color(u::text_muted()),
        );
        if u::link(ui, "The Movie Database") {
            u::open_url("https://www.themoviedb.org");
        }
    });
    if !shown {
        tui.settings_open = false;
        tui.token_seeded = false;
    }
    u::dialog_footer(ui, |ui| {
        if u::button(ui, "Close", &button_opts(ffi::cui_button_variant_CUI_BUTTON_SECONDARY, false)) {
            tui.settings_open = false;
        }
    });
}

// ---- scrape report ---------------------------------------------------------

/// One-shot report after a scrape with warnings (or always, if opened).
fn notice_dialog(ui: u::Ui, app: &mut App, message: &str) {
    let mut open = true;
    let mut dopts = unsafe { u::opts::<ffi::cui_dialog_opts>() };
    dopts.width = 650.0;
    let shown = u::dialog(ui, "Scrape report", &mut open, &dopts, |ui| {
        let mut scroll = unsafe { u::opts::<ffi::cui_scroll_opts>() };
        scroll.width = u::fill();
        scroll.height = u::px(300.0);
        u::scroll(ui, &scroll, |ui| {
            u::text(ui, message, &text_color(u::fg()));
        });
        u::dialog_footer(ui, |ui| {
            if u::button(ui, "Close", &button_opts(ffi::cui_button_variant_CUI_BUTTON_SECONDARY, false)) {
                open = false;
            }
        });
    });
    if !shown {
        app.tmdb_ui.notice = None;
    }
}
