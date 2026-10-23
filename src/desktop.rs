use eframe::egui::{self, Color32, RichText};
mod file_tools;
mod library_view;
mod matching;
mod season_view;
mod series_tree;
mod theme;
use media_shelf::storage::{LibraryDb, database_path};
use media_shelf::{Category, ScanReport, Source, Video};
use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    time::Duration,
};

pub fn run() -> eframe::Result {
    eframe::run_native(
        "Media Shelf",
        eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default()
                .with_inner_size([1380.0, 880.0])
                .with_min_inner_size([1080.0, 720.0]),
            ..Default::default()
        },
        Box::new(|cc| {
            egui_extras::install_image_loaders(&cc.egui_ctx);
            theme::install(&cc.egui_ctx);
            Ok(Box::new(App::new()))
        }),
    )
}

enum Event {
    Progress(usize),
    LocalReload(Result<(Vec<Video>, usize, Vec<String>), String>),
    Done(ScanReport, Result<Vec<Video>, String>),
}

struct App {
    sources: Vec<Source>,
    category: Category,
    folder_input: String,
    filter: String,
    videos: Vec<Video>,
    series_tree: series_tree::SeriesTree,
    warnings: Vec<String>,
    error: Option<String>,
    status: String,
    receiver: Option<mpsc::Receiver<Event>>,
    cancel: Arc<AtomicBool>,
    database: Option<LibraryDb>,
    selected: Option<PathBuf>,
    selected_season: Option<(PathBuf, u32)>,
    tmdb_ui: matching::TmdbUi,
    file_tools: file_tools::FileTools,
    selection: std::collections::HashSet<PathBuf>,
    detail_tab: usize,
    sources_open: bool,
    filter_mode: usize,
    sort: library_view::SortColumn,
    sort_descending: bool,
}

impl App {
    fn new() -> Self {
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
        let mut tmdb_ui = matching::TmdbUi::new();
        tmdb_ui.show_missing_episodes = database
            .as_ref()
            .and_then(|db| db.show_missing_episodes().ok())
            .unwrap_or(false);
        Self {
            sources,
            category,
            folder_input: String::new(),
            filter: String::new(),
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
            tmdb_ui,
            file_tools: file_tools::FileTools::new(),
            selection,
            detail_tab: 0,
            sources_open: false,
            filter_mode: 0,
            sort: library_view::SortColumn::Title,
            sort_descending: false,
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

    fn add_folder(&mut self, path: PathBuf) {
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

    fn reload_local_metadata(&mut self) {
        if self.database.is_none() || self.receiver.is_some() {
            return;
        }
        let paths: Vec<_> = self
            .selected_videos(false)
            .into_iter()
            .map(|v| v.path)
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

    fn start_scan(&mut self) {
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

    fn poll(&mut self) {
        let mut done = false;
        if let Some(receiver) = &self.receiver {
            loop {
                match receiver.try_recv() {
                    Ok(Event::LocalReload(result)) => {
                        match result {
                            Ok((videos, count, warnings)) => {
                                self.videos = videos;
                                self.series_tree = Default::default();
                                self.status = format!(
                                    "Reloaded local metadata for {count} item(s) · {} warnings",
                                    warnings.len()
                                );
                                self.warnings = warnings;
                            }
                            Err(e) => {
                                self.error = Some(format!("Could not reload local metadata: {e}"))
                            }
                        }
                        done = true;
                        break;
                    }
                    Ok(Event::Progress(n)) => {
                        self.status = format!("Scanning folders… {n} movies / show folders found")
                    }
                    Ok(Event::Done(report, saved)) => {
                        self.status = if report.cancelled {
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
                            Ok(videos) => self.videos = videos,
                            Err(e) => {
                                self.status =
                                    "Scan could not be saved · Showing previous library".into();
                                self.error = Some(format!("Database update failed: {e}"));
                            }
                        }
                        self.warnings = report.warnings;
                        done = true;
                        break;
                    }
                    Err(mpsc::TryRecvError::Empty) => break,
                    Err(mpsc::TryRecvError::Disconnected) => {
                        self.error =
                            Some("The scanner stopped unexpectedly. Try scanning again.".into());
                        done = true;
                        break;
                    }
                }
            }
        }
        if done {
            self.receiver = None;
        }
    }
}

impl Drop for App {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
    }
}

fn display_path(path: &std::path::Path) -> String {
    let path = path.to_string_lossy();
    if let Some(unc) = path.strip_prefix("\\\\?\\UNC\\") {
        format!("\\\\{unc}")
    } else {
        path.strip_prefix("\\\\?\\").unwrap_or(&path).to_string()
    }
}

fn size(bytes: u64) -> String {
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
