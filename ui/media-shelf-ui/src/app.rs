//! Application state, background work, and the CUI frame callback.
//!
//! `App` holds everything the UI needs. The frame callback (a plain C function
//! pointer, so no `Box<dyn>` / `Rc` inside the tree) reads the state through
//! `&mut *user`, draws one CUI frame, and processes background results.

use crate::{library, series};
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
    pub filter_mode: i32,
    pub sort: library::SortColumn,
    pub sort_descending: bool,
    /// Theme override; `None` follows the system.
    pub theme_dark: Option<bool>,
}

impl App {
    pub fn new() -> Self {
        let loaded = (|| -> media_shelf::storage::DbResult<_> {
            let path = database_path();
            let mut db = LibraryDb::open(&path)?;
            db.migrate_folders(&path.with_file_name("folders.json"))?;
            let sources = db.sources()?;
            let warnings = db.import_local_nfos()?;
            let videos = db.videos()?;
            Ok((db, sources, videos, warnings))
        })();
        let (database, sources, videos, warnings, error) = match loaded {
            Ok((db, sources, videos, warnings)) => (Some(db), sources, videos, warnings, None),
            Err(e) => (
                None,
                vec![],
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
        let selected = videos.first().map(|v| v.path.clone());
        let category = videos
            .first()
            .map(|v| v.category)
            .unwrap_or(Category::Movies);
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
            filter_mode: 0,
            sort: library::SortColumn::Title,
            sort_descending: false,
            theme_dark: None,
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
}

impl Drop for App {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
    }
}

/// Drain background events. Call once per frame.
pub fn poll(app: &mut App) {
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
    poll(app);
    render(ui, app);
}

fn render(ui: crate::ui::Ui, app: &mut App) {
    use crate::ui as u;

    if let Some(dark) = app.theme_dark {
        u::set_theme(ui, dark);
    }
    if app.category == Category::Movies {
        library::movies_grid(ui, app);
    } else {
        // Series browsing arrives with the series view (next task).
        u::text(ui, "Series browsing is on the way.", &unsafe { u::opts() });
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
