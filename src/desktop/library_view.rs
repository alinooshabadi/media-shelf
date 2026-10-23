use super::*;
use media_shelf::metadata;
use std::collections::{BTreeMap, HashSet};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum SortColumn {
    Title,
    Year,
    Rating,
    Status,
    Seasons,
    Episodes,
    Added,
}

fn folder_link(ui: &mut egui::Ui, label: &str, folder: &std::path::Path) -> Option<String> {
    let response = ui
        .add(
            egui::Label::new(RichText::new(label).color(theme::ACCENT))
                .truncate()
                .sense(egui::Sense::click()),
        )
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .on_hover_text(format!("Open containing folder\n{}", display_path(folder)));
    if !response.clicked() {
        return None;
    }
    if !folder.is_dir() {
        return Some("Folder is unavailable. Check that the drive is connected.".into());
    }
    #[cfg(windows)]
    let result = {
        use std::os::windows::process::CommandExt;
        let explorer =
            PathBuf::from(std::env::var_os("WINDIR").unwrap_or_else(|| "C:\\Windows".into()))
                .join("explorer.exe");
        std::process::Command::new(explorer)
            .arg(display_path(folder))
            .creation_flags(0x08000000)
            .spawn()
    };
    #[cfg(target_os = "macos")]
    let result = std::process::Command::new("/usr/bin/open")
        .arg(folder)
        .spawn();
    #[cfg(not(any(windows, target_os = "macos")))]
    let result = std::process::Command::new("xdg-open").arg(folder).spawn();
    result.err().map(|e| format!("Could not open folder: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture(video: Video) -> App {
        App {
            sources: vec![],
            category: video.category,
            folder_input: String::new(),
            filter: String::new(),
            videos: vec![video.clone()],
            series_tree: Default::default(),
            warnings: vec![],
            error: None,
            status: String::new(),
            receiver: None,
            cancel: Arc::new(AtomicBool::new(false)),
            database: None,
            selected: Some(video.path.clone()),
            selected_season: None,
            tmdb_ui: matching::TmdbUi::new(),
            file_tools: file_tools::FileTools::new(),
            selection: [video.path].into_iter().collect(),
            detail_tab: 0,
            sources_open: false,
            filter_mode: 0,
            sort: SortColumn::Title,
            sort_descending: false,
        }
    }

    #[test]
    fn existing_episode_title_lookup_is_independent_of_missing_episode_display() {
        let show = Video {
            added_date: None,
            is_folder: true,
            category: Category::Series,
            path: "/shows/The Rehearsal (2022)".into(),
            relative_path: "The Rehearsal (2022)".into(),
            bytes: 0,
            artwork: Default::default(),
            metadata: None,
            tmdb: Some(media_shelf::tmdb::TmdbMatch {
                category: Category::Series,
                id: 1,
                title: "The Rehearsal".into(),
                original_title: "The Rehearsal".into(),
                release_date: Some("2022-07-15".into()),
                overview: String::new(),
            }),
        };
        let mut app = fixture(show.clone());
        app.tmdb_ui.show_missing_episodes = false;
        assert!(
            app.episode_lookup(&show).is_some(),
            "Existing episode titles must load even when missing-episode display is disabled"
        );
    }

    #[test]
    fn columns_sort_both_directions_and_unknown_values_stay_last() {
        let identity = media_shelf::tmdb::TmdbMatch {
            category: Category::Series,
            id: 42,
            title: "Alpha".into(),
            original_title: "Alpha".into(),
            release_date: Some("2020-01-01".into()),
            overview: String::new(),
        };
        let mut metadata = media_shelf::metadata::Metadata::empty(identity.clone());
        metadata.year = Some(2020);
        metadata.rating = Some(9.0);
        let a = Video {
            added_date: Some("2026-10-22".into()),
            is_folder: true,
            category: Category::Series,
            path: "/shows/Alpha".into(),
            relative_path: "Alpha".into(),
            bytes: 0,
            tmdb: Some(identity),
            metadata: Some(metadata),
            artwork: Default::default(),
        };
        let mut b = a.clone();
        b.path = "/shows/Beta".into();
        b.tmdb.as_mut().unwrap().title = "Beta".into();
        b.metadata.as_mut().unwrap().year = Some(1999);
        b.metadata.as_mut().unwrap().rating = Some(2.0);
        b.added_date = Some("2000-01-01".into());
        let mut app = fixture(a.clone());
        for (column, expected) in [
            (SortColumn::Title, std::cmp::Ordering::Less),
            (SortColumn::Year, std::cmp::Ordering::Greater),
            (SortColumn::Rating, std::cmp::Ordering::Greater),
            (SortColumn::Added, std::cmp::Ordering::Greater),
        ] {
            app.sort = column;
            app.sort_descending = false;
            assert_eq!(app.compare_items(&a, &b), expected);
            app.sort_descending = true;
            assert_eq!(app.compare_items(&a, &b), expected.reverse());
        }
        b.added_date = None;
        for desc in [false, true] {
            app.sort_descending = desc;
            assert!(app.compare_items(&a, &b).is_lt());
        }
        b.metadata = None;
        app.sort = SortColumn::Status;
        app.sort_descending = false;
        assert!(app.compare_items(&a, &b).is_gt());
        app.sort_descending = true;
        assert!(app.compare_items(&a, &b).is_lt());
        let episode = series_tree::Episode {
            video: a.clone(),
            number: Some(1),
            needs_title: false,
            details: Default::default(),
            label: "1. Pilot".into(),
        };
        let mut one = series_tree::Show::default();
        one.seasons.insert(Some(1), vec![episode.clone()]);
        let mut two = series_tree::Show::default();
        two.seasons.insert(Some(1), vec![episode.clone()]);
        two.seasons.insert(Some(2), vec![episode]);
        app.series_tree.shows.insert(a.path.clone(), two);
        app.series_tree.shows.insert(b.path.clone(), one);
        for column in [SortColumn::Seasons, SortColumn::Episodes] {
            app.sort = column;
            app.sort_descending = false;
            assert!(app.compare_items(&a, &b).is_gt());
            app.sort_descending = true;
            assert!(app.compare_items(&a, &b).is_lt());
        }
    }

    #[test]
    fn expanded_show_tree_selects_episode_details_and_preserves_show_actions() {
        let temp = tempfile::tempdir().unwrap();
        let folder = temp.path().join("Show (2022)");
        let season = folder.join("Season 1");
        std::fs::create_dir_all(&season).unwrap();
        let path = season.join("Show - S01E01.mp4");
        std::fs::write(&path, b"video").unwrap();
        let thumb = season.join("Show - S01E01 - Pilot-thumb.jpg");
        image::RgbImage::from_pixel(16, 9, image::Rgb([40, 80, 120]))
            .save(&thumb)
            .unwrap();
        std::fs::write(season.join("Show - S01E01 - Pilot.nfo"),
            "<episodedetails><title>Pilot</title><season>1</season><episode>1</episode><aired>2022-07-15</aired><rating>7.8</rating></episodedetails>").unwrap();
        let video = Video {
            added_date: None,
            is_folder: true,
            category: Category::Series,
            path: folder.clone(),
            relative_path: "Show (2022)".into(),
            bytes: 0,
            tmdb: None,
            metadata: None,
            artwork: Default::default(),
        };
        let mut app = fixture(video.clone());
        app.series_tree
            .shows
            .insert(folder.clone(), series_tree::load(&folder, &[]));
        assert_eq!(
            app.series_tree
                .episode(&path)
                .unwrap()
                .video
                .artwork
                .thumb
                .as_ref(),
            Some(&thumb)
        );
        let ctx = egui::Context::default();
        theme::install(&ctx);
        let render = |app: &mut App, events: Vec<egui::Event>| {
            ctx.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(1080.0, 720.0),
                    )),
                    events,
                    ..Default::default()
                },
                |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| {
                        let id = ui.make_persistent_id(("indexed_show", &folder));
                        egui::collapsing_header::CollapsingState::load_with_default_open(
                            ctx, id, true,
                        )
                        .store(ctx);
                        app.indexed_show(ui, video.clone(), "Show");
                    });
                },
            )
        };
        let mut episode_pos = None;
        let mut season_pos = None;
        for _ in 0..3 {
            for shape in render(&mut app, vec![]).shapes {
                if let egui::Shape::Text(text) = shape.shape {
                    if text.galley.text().starts_with("Season 1") {
                        season_pos = Some(text.pos + text.galley.size() / 2.0);
                    }
                    if text.galley.text() == "1. Pilot" {
                        episode_pos = Some(text.pos + text.galley.size() / 2.0);
                    }
                }
            }
        }
        let season_pos = season_pos.expect("Expanded tree must display the season header");
        for pressed in [true, false] {
            render(
                &mut app,
                vec![
                    egui::Event::PointerMoved(season_pos),
                    egui::Event::PointerButton {
                        pos: season_pos,
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers: Default::default(),
                    },
                ],
            );
        }
        assert_eq!(app.selected_season, Some((folder.clone(), 1)));
        assert_eq!(app.selected_videos(false)[0].path, folder);
        app.sources.push(Source {
            category: Category::Series,
            path: temp.path().to_path_buf(),
        });
        app.videos[0].is_folder = false;
        app.videos[0].path = path.clone();
        assert_eq!(
            app.selected_videos(false)[0].path,
            path,
            "Season actions also support a saved episode index"
        );
        app.videos[0] = video.clone();
        let output = ctx.run(Default::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| app.detail_view(ui));
        });
        for label in [
            "Episode",
            "Title",
            "Aired",
            "Rating",
            "Pilot",
            "2022-07-15",
            "7.8",
        ] {
            assert!(
                output
                    .shapes
                    .iter()
                    .any(|s| matches!(&s.shape, egui::Shape::Text(t) if t.galley.text() == label)),
                "Missing {label}"
            );
        }
        let title_pos = output
            .shapes
            .iter()
            .find_map(|s| match &s.shape {
                egui::Shape::Text(t) if t.galley.text() == "Pilot" => {
                    Some(t.pos + t.galley.size() / 2.0)
                }
                _ => None,
            })
            .unwrap();
        for pressed in [true, false] {
            let _ = ctx.run(
                egui::RawInput {
                    events: vec![
                        egui::Event::PointerMoved(title_pos),
                        egui::Event::PointerButton {
                            pos: title_pos,
                            button: egui::PointerButton::Primary,
                            pressed,
                            modifiers: Default::default(),
                        },
                    ],
                    ..Default::default()
                },
                |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| app.detail_view(ui));
                },
            );
        }
        assert_eq!(app.selected, Some(path.clone()));
        assert!(
            app.selected_season.is_none(),
            "Clicking the table title opens episode details"
        );
        render(&mut app, vec![]);
        let pos = episode_pos.expect("Expanded tree must display the NFO episode title");
        render(
            &mut app,
            vec![
                egui::Event::PointerMoved(pos),
                egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: Default::default(),
                },
            ],
        );
        render(
            &mut app,
            vec![egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: Default::default(),
            }],
        );
        assert_eq!(app.selected, Some(path.clone()));
        assert!(app.selected_season.is_none());
        assert_eq!(
            app.detail_tab, 2,
            "Selecting an episode opens its media file details"
        );
        assert_eq!(
            app.selected_videos(false)[0].path,
            folder,
            "Scrape and organize must still target the indexed show"
        );
        let output = ctx.run(Default::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| app.detail_view(ui));
        });
        assert!(output.shapes.iter().any(
            |s| matches!(&s.shape,egui::Shape::Text(t) if t.galley.text() == "Show - S01E01.mp4")
        ));
    }

    #[test]
    fn light_library_and_detail_tabs_render_at_supported_sizes() {
        for (width, category) in [
            (1080.0, Category::Movies),
            (1380.0, Category::Movies),
            (1080.0, Category::Series),
            (1380.0, Category::Series),
        ] {
            let identity = media_shelf::tmdb::TmdbMatch {
                category,
                id: 42,
                title: "A Movie with a Longer Title".into(),
                original_title: "Original title".into(),
                release_date: Some("2001-03-07".into()),
                overview: "A sample overview for this movie.".into(),
            };
            let mut metadata = media_shelf::metadata::Metadata::empty(identity.clone());
            metadata.year = Some(2001);
            metadata.studios = vec!["A long production company name ".repeat(15)];
            let video = Video {
                added_date: None,
                is_folder: category == Category::Series,
                artwork: Default::default(),
                category,
                path: if category == Category::Movies {
                    "C:/fixtures/movie.mp4"
                } else {
                    "C:/fixtures/Show (2001)"
                }
                .into(),
                relative_path: "movie.mp4".into(),
                bytes: 1024,
                tmdb: Some(identity),
                metadata: Some(metadata),
            };
            let mut app = fixture(video.clone());
            let ctx = egui::Context::default();
            theme::install(&ctx);
            assert!(!ctx.style().visuals.dark_mode);
            for tab in 0..5 {
                app.detail_tab = tab;
                for _ in 0..3 {
                    let output = ctx.run(
                        egui::RawInput {
                            screen_rect: Some(egui::Rect::from_min_size(
                                egui::Pos2::ZERO,
                                egui::vec2(width, 720.0),
                            )),
                            ..Default::default()
                        },
                        |ctx| {
                            egui::SidePanel::left("test_library")
                                .exact_width(440.0)
                                .show(ctx, |ui| app.library_list(ui));
                            egui::CentralPanel::default().show(ctx, |ui| app.detail_view(ui));
                        },
                    );
                    assert!(!output.shapes.is_empty());
                    for clipped in &output.shapes {
                        if let egui::Shape::Text(text) = &clipped.shape {
                            if text
                                .galley
                                .job
                                .text
                                .starts_with("A long production company")
                            {
                                let rect = text.galley.rect.translate(text.pos.to_vec2());
                                assert!(
                                    rect.max.x <= width + 1.0,
                                    "Studio names must wrap inside the detail pane"
                                );
                            }
                        }
                    }
                }
            }
            assert_eq!(app.videos[0].is_folder, category == Category::Series);
        }
    }
}

#[derive(Clone, Copy)]
enum ItemAction {
    Scrape,
    Rename,
    ReloadLocal,
}

fn item_menu(response: &egui::Response, locked: bool) -> Option<ItemAction> {
    let mut action = None;
    response.context_menu(|ui| {
        ui.add_enabled_ui(!locked, |ui| {
            if ui.button("Search & scrape…").clicked() {
                action = Some(ItemAction::Scrape);
                ui.close();
            }
            if ui.button("Reload local metadata").clicked() {
                action = Some(ItemAction::ReloadLocal);
                ui.close();
            }
            if ui.button("Rename…").clicked() {
                action = Some(ItemAction::Rename);
                ui.close();
            }
        });
    });
    action
}

pub(super) fn title(v: &Video) -> String {
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
fn episode_label(v: &Video) -> String {
    v.path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned()
}
impl App {
    fn item_actions_locked(&self) -> bool {
        self.receiver.is_some() || self.tmdb_ui.active() || self.file_tools.active()
    }

    fn apply_item_context(&mut self, paths: Vec<PathBuf>, action: Option<ItemAction>) {
        // Right-clicking a selected row keeps the multi-selection intact.
        if !paths
            .iter()
            .any(|p| self.selection.contains(p) || self.selected.as_ref() == Some(p))
        {
            self.selection.clear();
            self.selection.extend(paths.iter().cloned());
        }
        self.selected = paths.first().cloned();
        self.selected_season = None;
        self.detail_tab = 0;
        if self.item_actions_locked() {
            return;
        }
        match action {
            Some(ItemAction::ReloadLocal) => self.reload_local_metadata(),
            Some(ItemAction::Scrape) => self.tmdb_ui.open_many(self.selected_videos(false), false),
            Some(ItemAction::Rename) => {
                let videos: Vec<_> = self
                    .videos
                    .iter()
                    .filter(|v| v.category == self.category && self.library_item_selected(v))
                    .cloned()
                    .collect();
                if videos.len() == 1 {
                    self.file_tools
                        .rename(videos[0].clone(), self.sources.clone());
                } else if !videos.is_empty() {
                    self.file_tools.batch(videos, self.sources.clone());
                }
            }
            None => {}
        }
    }
    pub(super) fn selected_videos(&self, unscraped: bool) -> Vec<Video> {
        let mut groups = HashSet::new();
        self.videos
            .iter()
            .filter(|v| {
                v.category == self.category
                    && (if unscraped {
                        v.metadata.is_none()
                    } else {
                        self.library_item_selected(v)
                    })
            })
            .filter(|v| {
                let key =
                    metadata::series_folder(v, &self.sources).unwrap_or_else(|| v.path.clone());
                groups.insert(key)
            })
            .cloned()
            .collect()
    }
    pub(super) fn select(&mut self, path: PathBuf, additive: bool) {
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
    fn organize_menu(&mut self, ui: &mut egui::Ui) {
        ui.menu_button("Rename & cleanup", |ui| {
            let selected = self
                .selected
                .as_ref()
                .and_then(|p| {
                    self.videos.iter().find(|v| {
                        v.category == self.category
                            && (&v.path == p || (v.is_folder && p.starts_with(&v.path)))
                    })
                })
                .cloned();
            if ui
                .add_enabled(
                    selected.is_some(),
                    egui::Button::new(if self.category == Category::Series {
                        "Rename selected show…"
                    } else {
                        "Rename selected file…"
                    }),
                )
                .clicked()
            {
                self.file_tools
                    .rename(selected.unwrap(), self.sources.clone());
                ui.close();
            }
            if ui.button("Rename all matched files…").clicked() {
                self.file_tools.batch(
                    self.videos
                        .iter()
                        .filter(|v| v.category == self.category)
                        .cloned()
                        .collect(),
                    self.sources.clone(),
                );
                ui.close();
            }
            if ui.button("Preview folder cleanup…").clicked() {
                self.file_tools.cleanup(self.sources.clone(), self.category);
                ui.close();
            }
            ui.separator();
            if ui.button("Undo / recover last operation…").clicked() {
                self.file_tools.undo();
                ui.close();
            }
        });
    }
    fn sources_window(&mut self, ctx: &egui::Context, locked: bool) {
        if !self.sources_open {
            return;
        }
        let mut open = true;
        egui::Window::new("Data sources")
            .open(&mut open)
            .default_width(650.0)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.selectable_value(&mut self.category, Category::Movies, "Movies");
                    ui.selectable_value(&mut self.category, Category::Series, "TV shows");
                });
                ui.label("Add root folders. Movies scan recursively; TV shows load episodes when expanded.");
                ui.add_enabled_ui(!locked, |ui| {
                    if ui.button("Browse for folder…").clicked()
                        && let Some(path) = rfd::FileDialog::new().pick_folder()
                    {
                        self.add_folder(path);
                    }
                    ui.horizontal(|ui| {
                        ui.add(
                            egui::TextEdit::singleline(&mut self.folder_input)
                                .desired_width(470.0)
                                .hint_text("Or paste a folder path"),
                        );
                        if ui.button("Add").clicked() {
                            self.add_folder(PathBuf::from(
                                self.folder_input.trim().trim_matches('"'),
                            ));
                        }
                    });
                });
                let mut remove = None;
                for (index, s) in self
                    .sources
                    .iter()
                    .enumerate()
                    .filter(|(_, s)| s.category == self.category)
                {
                    ui.horizontal(|ui| {
                        if let Some(error) = folder_link(ui, &display_path(&s.path), &s.path) {
                            self.error = Some(error);
                        }
                        if ui
                            .add_enabled(!locked, egui::Button::new("Remove"))
                            .clicked()
                        {
                            remove = Some(index);
                        }
                    });
                }
                if let Some(index) = remove {
                    let mut sources = self.sources.clone();
                    sources.remove(index);
                    if self.save_sources(sources) {
                        self.start_scan();
                    }
                }
                if let Some(error) = &self.error {
                    ui.colored_label(Color32::from_rgb(170, 35, 35), error);
                }
            });
        self.sources_open = open;
    }
    fn sort_header(&mut self, ui: &mut egui::Ui, column: SortColumn, label: &str) {
        let text = if self.sort == column {
            format!("{label} {}", if self.sort_descending { "v" } else { "^" })
        } else {
            label.to_string()
        };
        let hint = match column {
            SortColumn::Added => {
                "Date first added to this library (local date). Rescans and renames keep it."
            }
            SortColumn::Seasons | SortColumn::Episodes => {
                "Local counts load when a show is expanded. Unloaded counts sort last."
            }
            _ => "Click to reverse the sort order.",
        };
        if ui
            .add(
                egui::Label::new(text)
                    .truncate()
                    .sense(egui::Sense::click()),
            )
            .on_hover_cursor(egui::CursorIcon::PointingHand)
            .on_hover_text(hint)
            .clicked()
        {
            if self.sort == column {
                self.sort_descending = !self.sort_descending;
            } else {
                self.sort = column;
                self.sort_descending = false;
            }
        }
    }

    fn compare_items(&self, a: &Video, b: &Video) -> std::cmp::Ordering {
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
        let order = match self.sort {
            SortColumn::Title => optional(
                Some(title(a).to_lowercase()),
                Some(title(b).to_lowercase()),
                self.sort_descending,
            ),
            SortColumn::Year => optional(
                Some(year(a)).filter(|s| !s.is_empty()),
                Some(year(b)).filter(|s| !s.is_empty()),
                self.sort_descending,
            ),
            SortColumn::Rating => optional(
                a.metadata
                    .as_ref()
                    .and_then(|m| m.rating)
                    .filter(|r| r.is_finite())
                    .map(|r| (r * 1000.0) as i64),
                b.metadata
                    .as_ref()
                    .and_then(|m| m.rating)
                    .filter(|r| r.is_finite())
                    .map(|r| (r * 1000.0) as i64),
                self.sort_descending,
            ),
            SortColumn::Status => optional(
                Some((a.metadata.is_some(), a.tmdb.is_some())),
                Some((b.metadata.is_some(), b.tmdb.is_some())),
                self.sort_descending,
            ),
            SortColumn::Added => optional(
                a.added_date.as_ref(),
                b.added_date.as_ref(),
                self.sort_descending,
            ),
            SortColumn::Seasons | SortColumn::Episodes => {
                let count = |v: &Video| {
                    self.series_tree.shows.get(&v.path).map(|s| {
                        if self.sort == SortColumn::Seasons {
                            s.season_count()
                        } else {
                            s.count()
                        }
                    })
                };
                optional(count(a), count(b), self.sort_descending)
            }
        };
        order
            .then_with(|| title(a).to_lowercase().cmp(&title(b).to_lowercase()))
            .then_with(|| a.path.cmp(&b.path))
    }

    fn library_list(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.add(
                egui::TextEdit::singleline(&mut self.filter)
                    .hint_text("Search library…")
                    .desired_width((ui.available_width() - 115.0).max(120.0)),
            );
            egui::ComboBox::from_id_salt("filter")
                .width(65.0)
                .selected_text(["All", "Unscraped", "No artwork"][self.filter_mode])
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.filter_mode, 0, "All");
                    ui.selectable_value(&mut self.filter_mode, 1, "Unscraped");
                    ui.selectable_value(&mut self.filter_mode, 2, "No artwork");
                });
        });
        let filter = self.filter.to_lowercase();
        let mut entries: Vec<usize> = self
            .videos
            .iter()
            .enumerate()
            .filter(|(_, v)| {
                v.category == self.category
                    && (filter.is_empty()
                        || title(v).to_lowercase().contains(&filter)
                        || v.path.to_string_lossy().to_lowercase().contains(&filter))
                    && match self.filter_mode {
                        1 => v.metadata.is_none(),
                        2 => {
                            v.artwork.poster.is_none()
                                && v.metadata.as_ref().is_none_or(|m| m.poster_file.is_none())
                        }
                        _ => true,
                    }
            })
            .map(|(i, _)| i)
            .collect();
        entries.sort_by(|a, b| self.compare_items(&self.videos[*a], &self.videos[*b]));
        ui.horizontal(|ui| {
            ui.weak(format!("{} items", entries.len()));
            if ui.small_button("Select visible").clicked() {
                self.selection = entries
                    .iter()
                    .map(|i| self.videos[*i].path.clone())
                    .collect();
                self.selected = entries.first().map(|i| self.videos[*i].path.clone());
                self.selected_season = None;
            }
            if ui.small_button("Clear selection").clicked() {
                self.selection.clear();
                self.selected = None;
                self.selected_season = None;
            }
        });
        if self.category == Category::Series {
            self.show_tree(ui, &entries);
            return;
        }
        let mut click = None;
        let mut context = None;
        let locked = self.item_actions_locked();
        egui_extras::TableBuilder::new(ui)
            .striped(false)
            .sense(egui::Sense::click())
            .resizable(false)
            .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
            .column(egui_extras::Column::remainder().at_least(120.0))
            .column(egui_extras::Column::initial(38.0))
            .column(egui_extras::Column::initial(60.0))
            .column(egui_extras::Column::exact(66.0))
            .column(egui_extras::Column::exact(82.0))
            .header(36.0, |mut row| {
                for (column, label) in [
                    (SortColumn::Title, "Title"),
                    (SortColumn::Year, "Year"),
                    (SortColumn::Rating, "Rating"),
                    (SortColumn::Status, "Status"),
                    (SortColumn::Added, "Added"),
                ] {
                    row.col(|ui| {
                        self.sort_header(ui, column, label);
                    });
                }
            })
            .body(|body| {
                body.rows(44.0, entries.len(), |mut row| {
                    let video = &self.videos[entries[row.index()]];
                    let selected = self.selection.contains(&video.path)
                        || self.selected.as_ref() == Some(&video.path);
                    row.set_selected(selected);
                    row.col(|ui| {
                        let response = ui
                            .add(
                                egui::Label::new(RichText::new(title(video)).size(15.0))
                                    .truncate()
                                    .sense(egui::Sense::click()),
                            )
                            .on_hover_text(display_path(&video.path));
                        if response.clicked() {
                            click = Some((
                                video.path.clone(),
                                ui.input(|i| i.modifiers.ctrl || i.modifiers.command),
                                response.double_clicked(),
                            ));
                        }
                    });
                    row.col(|ui| {
                        ui.label(year(video));
                    });
                    row.col(|ui| {
                        ui.label(
                            video
                                .metadata
                                .as_ref()
                                .and_then(|m| m.rating)
                                .map(|r| format!("{r:.1}"))
                                .unwrap_or_default(),
                        );
                    });
                    row.col(|ui| {
                        let (label, color) = if video.metadata.is_some() {
                            ("Scraped", Color32::from_rgb(60, 125, 102))
                        } else if video.tmdb.is_some() {
                            ("Matched", theme::ACCENT)
                        } else {
                            ("New", theme::MUTED)
                        };
                        let response = ui.label(RichText::new(label).size(12.0).color(color));
                        response.on_hover_text(format!(
                            "Metadata: {}\nCached poster: {}\nNFO exported: {}",
                            video.metadata.is_some(),
                            video
                                .metadata
                                .as_ref()
                                .is_some_and(|m| m.poster_file.is_some())
                                || video.artwork.poster.is_some(),
                            video.metadata.as_ref().is_some_and(|m| m.nfo_written)
                        ));
                    });
                    row.col(|ui| {
                        ui.label(video.added_date.as_deref().unwrap_or("–"));
                    });
                    let response = row.response();
                    if response.clicked() {
                        click = Some((
                            video.path.clone(),
                            response
                                .ctx
                                .input(|i| i.modifiers.ctrl || i.modifiers.command),
                            response.double_clicked(),
                        ));
                    }
                    let action = item_menu(&response, locked);
                    if response.secondary_clicked() || action.is_some() {
                        context = Some((vec![video.path.clone()], action));
                    }
                });
            });
        if let Some((path, additive, double)) = click {
            self.select(path, additive);
            if double
                && !self.tmdb_ui.active()
                && !self.file_tools.active()
                && self.receiver.is_none()
            {
                self.tmdb_ui.open_many(self.selected_videos(false), false);
            }
        }
        if let Some((paths, action)) = context {
            self.apply_item_context(paths, action);
        }
    }
    fn episode_lookup(&self, show: &Video) -> Option<(i64, String, String)> {
        show.tmdb.as_ref().map(|m| {
            let (token, language) = self.tmdb_ui.browsing_credentials();
            (m.id, token, language)
        })
    }

    fn indexed_show(&mut self, ui: &mut egui::Ui, show: Video, name: &str) {
        let folder = &show.path;
        let counts = self
            .series_tree
            .shows
            .get(folder)
            .map(|s| (s.season_count(), s.count()));
        let mut click = None;
        let mut season_click = None;
        let mut retry = false;
        let mut context = None;
        let locked = self.item_actions_locked();
        let catalog = self.episode_lookup(&show);
        let id = ui.make_persistent_id(("indexed_show", folder));
        let (_, header, _) =
            egui::collapsing_header::CollapsingState::load_with_default_open(ui.ctx(), id, false)
                .show_header(ui, |ui| {
                    let response = ui
                        .add_sized(
                            [
                                (ui.available_width() - 180.0 - 4.0 * ui.spacing().item_spacing.x)
                                    .max(40.0),
                                22.0,
                            ],
                            egui::Button::selectable(
                                self.selected.as_ref() == Some(folder),
                                (name, egui::Atom::grow()),
                            )
                            .truncate(),
                        )
                        .on_hover_text(display_path(folder));
                    ui.add_sized([42.0, 22.0], egui::Label::new(year(&show)));
                    ui.add_sized(
                        [26.0, 22.0],
                        egui::Label::new(counts.map_or("–".into(), |(s, _)| s.to_string())),
                    );
                    ui.add_sized(
                        [30.0, 22.0],
                        egui::Label::new(counts.map_or("–".into(), |(_, e)| e.to_string())),
                    );
                    ui.add_sized(
                        [82.0, 22.0],
                        egui::Label::new(show.added_date.as_deref().unwrap_or("–")),
                    );
                    response
                })
                .body(|ui| {
                    self.series_tree.request(
                        folder,
                        &self.sources,
                        catalog.clone(),
                        self.tmdb_ui.show_missing_episodes,
                    );
                    let Some(loaded) = self.series_tree.shows.get(folder) else {
                        ui.horizontal(|ui| {
                            ui.spinner();
                            ui.weak("Loading episodes…");
                        });
                        return;
                    };
                    if loaded.count() == 0 && loaded.warnings.is_empty() {
                        ui.weak("No video files found.");
                    }
                    if loaded.checking_metadata {
                        ui.horizontal(|ui| {
                            ui.spinner();
                            ui.weak(if self.tmdb_ui.show_missing_episodes {
                                "Checking missing episodes on TMDB…"
                            } else {
                                "Loading episode details from TMDB…"
                            });
                        });
                    } else if self.tmdb_ui.show_missing_episodes && show.tmdb.is_none() {
                        ui.weak("Match this show to check for missing seasons and episodes.");
                    }
                    for (season, episodes) in &loaded.seasons {
                        let name = match season {
                            Some(0) => "Specials".to_string(),
                            Some(n) => format!("Season {n}"),
                            None => "Other files".to_string(),
                        };
                        let missing = season.and_then(|s| loaded.missing.get(&s));
                        let missing_count = missing.map_or(0, |m| m.len());
                        let name = if episodes.is_empty() && missing_count > 0 {
                            format!("{name} · Missing")
                        } else {
                            name
                        };
                        let selected = season.is_some_and(|n| {
                            self.selected_season.as_ref() == Some(&(folder.clone(), n))
                        });
                        let id = ui.make_persistent_id((folder, season));
                        let (_, header, _) =
                            egui::collapsing_header::CollapsingState::load_with_default_open(
                                ui.ctx(),
                                id,
                                true,
                            )
                            .show_header(ui, |ui| {
                                ui.add(egui::Button::selectable(
                                    selected,
                                    if missing_count > 0 {
                                        format!(
                                            "{name}  ·  {} local / {missing_count} missing",
                                            episodes.len()
                                        )
                                    } else {
                                        format!("{name}  ·  {} episodes", episodes.len())
                                    },
                                ))
                            })
                            .body(|ui| {
                                let mut rows: Vec<_> = episodes
                                    .iter()
                                    .map(|e| (e.number.unwrap_or(u32::MAX), Some(e), None))
                                    .collect();
                                if let Some(missing) = missing {
                                    rows.extend(missing.iter().map(|(n, t)| (*n, None, Some(t))));
                                }
                                rows.sort_by_key(|(n, _, _)| *n);
                                for (number, episode, missing_title) in rows {
                                    if let Some(episode) = episode {
                                        let response = ui
                                            .selectable_label(
                                                self.selection.contains(&episode.video.path),
                                                &episode.label,
                                            )
                                            .on_hover_text(display_path(&episode.video.path));
                                        if response.clicked() {
                                            click = Some((
                                                episode.video.path.clone(),
                                                ui.input(|i| {
                                                    i.modifiers.ctrl || i.modifiers.command
                                                }),
                                            ));
                                        }
                                    } else if let Some(title) = missing_title {
                                        ui.colored_label(
                                            Color32::from_rgb(170, 85, 55),
                                            format!("{number}. {title} · Missing"),
                                        );
                                    }
                                }
                            });
                        if header.inner.clicked() {
                            if let Some(season) = season {
                                season_click = Some(*season);
                            }
                        }
                    }
                    if !loaded.warnings.is_empty() {
                        ui.colored_label(
                            Color32::from_rgb(170, 35, 35),
                            "Episode check needs attention.",
                        )
                        .on_hover_text(loaded.warnings.join("\n"));
                    }
                    retry = ui.small_button("Refresh episodes").clicked();
                });
        let action = item_menu(&header.inner, locked);
        if header.inner.secondary_clicked() || action.is_some() {
            context = Some(action);
        }
        if header.inner.clicked() {
            self.select(
                folder.clone(),
                ui.input(|i| i.modifiers.ctrl || i.modifiers.command),
            );
        }
        if retry {
            self.series_tree.shows.remove(folder);
            self.series_tree.request(
                folder,
                &self.sources,
                catalog.clone(),
                self.tmdb_ui.show_missing_episodes,
            );
        }
        if let Some(season) = season_click {
            self.select_season(folder.clone(), season);
        }
        if let Some((path, additive)) = click {
            self.select(path, additive);
            self.detail_tab = 2;
        }
        if let Some(action) = context {
            self.apply_item_context(vec![folder.clone()], action);
        }
    }

    // Show-level actions include selected descendants without adding episodes to the saved index.
    fn library_item_selected(&self, video: &Video) -> bool {
        (video.category == Category::Series
            && self.selected_season.as_ref().is_some_and(|(folder, _)| {
                metadata::series_folder(video, &self.sources).as_ref() == Some(folder)
            }))
            || self.selection.contains(&video.path)
            || self.selected.as_ref() == Some(&video.path)
            || (video.is_folder
                && self
                    .selection
                    .iter()
                    .chain(self.selected.iter())
                    .any(|p| p != &video.path && p.starts_with(&video.path)))
    }

    fn show_tree(&mut self, ui: &mut egui::Ui, entries: &[usize]) {
        let mut groups: BTreeMap<PathBuf, Vec<usize>> = BTreeMap::new();
        for i in entries {
            groups
                .entry(
                    metadata::series_folder(&self.videos[*i], &self.sources)
                        .unwrap_or_else(|| self.videos[*i].path.clone()),
                )
                .or_default()
                .push(*i);
        }
        let mut groups: Vec<_> = groups.into_iter().collect();
        groups.sort_by(|(_, a), (_, b)| self.compare_items(&self.videos[a[0]], &self.videos[b[0]]));
        let mut click = None;
        let mut show_click = None;
        let mut season_click = None;
        let mut context = None;
        let locked = self.item_actions_locked();
        ui.horizontal(|ui| {
            ui.add_space(ui.spacing().indent);
            let width =
                (ui.available_width() - 180.0 - 4.0 * ui.spacing().item_spacing.x).max(40.0);
            ui.allocate_ui_with_layout(
                egui::vec2(width, 22.0),
                egui::Layout::left_to_right(egui::Align::Center),
                |ui| {
                    ui.set_min_width(width);
                    self.sort_header(ui, SortColumn::Title, "Title");
                },
            );
            for (width, column, label) in [
                (42.0, SortColumn::Year, "Year"),
                (26.0, SortColumn::Seasons, "S"),
                (30.0, SortColumn::Episodes, "E"),
                (82.0, SortColumn::Added, "Added"),
            ] {
                ui.allocate_ui_with_layout(
                    egui::vec2(width, 22.0),
                    egui::Layout::left_to_right(egui::Align::Center),
                    |ui| {
                        ui.set_min_width(width);
                        self.sort_header(ui, column, label);
                    },
                );
            }
        });
        ui.separator();
        egui::ScrollArea::vertical()
            .id_salt("show_tree")
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 2.0;
                ui.spacing_mut().interact_size.y = 22.0;
                ui.spacing_mut().button_padding = egui::vec2(4.0, 2.0);

                for (folder, items) in groups {
                    let first = &self.videos[items[0]];
                    let name = first
                        .tmdb
                        .as_ref()
                        .map(|m| m.title.clone())
                        .unwrap_or_else(|| {
                            folder
                                .file_name()
                                .unwrap_or_default()
                                .to_string_lossy()
                                .into_owned()
                        });
                    if first.is_folder {
                        self.indexed_show(ui, first.clone(), &name);
                        continue;
                    }
                    let header =
                        egui::CollapsingHeader::new(format!("{name}  ({} files)", items.len()))
                            .id_salt(&folder)
                            .default_open(false)
                            .show(ui, |ui| {
                                let mut seasons: BTreeMap<String, Vec<usize>> = BTreeMap::new();
                                for i in &items {
                                    let v = &self.videos[*i];
                                    let parent = v.path.parent().unwrap_or(&folder);
                                    let season = if parent == folder {
                                        "Episodes".into()
                                    } else {
                                        parent
                                            .file_name()
                                            .unwrap_or_default()
                                            .to_string_lossy()
                                            .into_owned()
                                    };
                                    seasons.entry(season).or_default().push(*i);
                                }
                                for (season, files) in seasons {
                                    let season_number = files.iter().find_map(|i| {
                                        media_shelf::file_ops::episode_numbers(
                                            &self.videos[*i].path,
                                        )
                                        .map(|(s, _)| s)
                                    });
                                    let season_header = egui::CollapsingHeader::new(season)
                                        .id_salt((&folder, &files))
                                        .default_open(true)
                                        .show(ui, |ui| {
                                            for i in files {
                                                let v = &self.videos[i];
                                                let response = ui.selectable_label(
                                                    self.selected.as_ref() == Some(&v.path),
                                                    episode_label(v),
                                                );
                                                let action = item_menu(&response, locked);
                                                if response.secondary_clicked() || action.is_some()
                                                {
                                                    context = Some((vec![v.path.clone()], action));
                                                }
                                                if response.clicked() {
                                                    click = Some((
                                                        v.path.clone(),
                                                        ui.input(|i| {
                                                            i.modifiers.ctrl || i.modifiers.command
                                                        }),
                                                    ));
                                                }
                                            }
                                        });
                                    if season_header.header_response.clicked() {
                                        if let Some(n) = season_number {
                                            season_click = Some((folder.clone(), n, items[0]));
                                        }
                                    }
                                }
                            });
                    if header.header_response.clicked() {
                        show_click = Some((
                            items
                                .iter()
                                .map(|i| self.videos[*i].path.clone())
                                .collect::<Vec<_>>(),
                            ui.input(|i| i.modifiers.ctrl || i.modifiers.command),
                        ));
                    }
                    let action = item_menu(&header.header_response, locked);
                    if header.header_response.secondary_clicked() || action.is_some() {
                        context = Some((
                            items.iter().map(|i| self.videos[*i].path.clone()).collect(),
                            action,
                        ));
                    }
                }
            });
        if let Some((folder, season, index)) = season_click {
            let lookup = self.episode_lookup(&self.videos[index]);
            self.series_tree.request(
                &folder,
                &self.sources,
                lookup,
                self.tmdb_ui.show_missing_episodes,
            );
            self.select_season(folder, season);
        }
        if let Some((paths, additive)) = show_click {
            if !additive {
                self.selection.clear();
            }
            self.selected = paths.first().cloned();
            self.selected_season = None;
            self.selection.extend(paths);
            self.detail_tab = 0;
        }
        if let Some((path, additive)) = click {
            self.select(path, additive);
        }
        if let Some((paths, action)) = context {
            self.apply_item_context(paths, action);
        }
    }
    fn detail_view(&mut self, ui: &mut egui::Ui) {
        if let Some((folder, season)) = self.selected_season.clone() {
            self.season_view(ui, &folder, season);
            return;
        }
        let Some(video) = self
            .selected
            .as_ref()
            .and_then(|p| {
                self.videos
                    .iter()
                    .find(|v| v.category == self.category && &v.path == p)
                    .or_else(|| self.series_tree.episode(p).map(|e| &e.video))
            })
            .cloned()
        else {
            ui.add_space(100.0);
            ui.vertical_centered(|ui| {
                ui.heading("Select a movie or TV show");
                ui.label("Update sources, select items, then Search & scrape.");
            });
            return;
        };
        if let Some(episode) = self.series_tree.episode(&video.path) {
            ui.heading(&episode.label);
            ui.add_space(8.0);
        }
        ui.horizontal(|ui| {
            for (index, label) in [
                "Overview",
                "Cast & crew",
                "Media files",
                "Artwork",
                "Trailer",
            ]
            .iter()
            .enumerate()
            {
                ui.selectable_value(&mut self.detail_tab, index, *label);
            }
        });
        ui.add_space(20.0);
        egui::ScrollArea::vertical()
            .id_salt("detail_scroll")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                let data = video.metadata.as_ref();
                match self.detail_tab {
                    1 => {
                        ui.heading("Cast & crew");
                        ui.add_space(12.0);
                        if let Some(data) = data {
                            egui::Grid::new("people")
                                .num_columns(2)
                                .spacing([32.0, 14.0])
                                .show(ui, |ui| {
                                    for p in data.crew.iter().chain(data.cast.iter()) {
                                        ui.label(&p.name);
                                        ui.weak(&p.role);
                                        ui.end_row();
                                    }
                                });
                        } else {
                            ui.weak("Search & scrape to add cast and crew.");
                        }
                    }
                    2 => {
                        ui.heading("Media files");
                        ui.add_space(12.0);
                        if video.is_folder {
                            ui.weak(
                                "Expand this show in the library to browse seasons and episodes.",
                            );
                            if let Some(error) =
                                folder_link(ui, &display_path(&video.path), &video.path)
                            {
                                self.error = Some(error);
                            }
                            return;
                        }
                        if video.artwork.thumb.is_some() {
                            let width = ui.available_width().min(480.0);
                            matching::video_poster(
                                ui,
                                &video,
                                false,
                                egui::vec2(width, width * 9.0 / 16.0),
                            );
                            ui.add_space(12.0);
                        }
                        ui.label(episode_label(&video));
                        ui.horizontal(|ui| {
                            theme::badge(ui, &size(video.bytes));
                            theme::badge(
                                ui,
                                &video
                                    .path
                                    .extension()
                                    .unwrap_or_default()
                                    .to_string_lossy()
                                    .to_uppercase(),
                            );
                        });
                        ui.add_space(12.0);
                        if let Some(folder) = if video.is_folder {
                            Some(video.path.as_path())
                        } else {
                            video.path.parent()
                        } {
                            if let Some(error) = folder_link(ui, &display_path(&video.path), folder)
                            {
                                self.error = Some(error);
                            }
                        }
                        if let Some(folder) = metadata::series_folder(&video, &self.sources) {
                            if let Some(error) = folder_link(
                                ui,
                                &format!("Show folder: {}", display_path(&folder)),
                                &folder,
                            ) {
                                self.error = Some(error);
                            }
                        }
                        if let Some(m) = data {
                            ui.weak(if m.nfo_written {
                                "NFO exported"
                            } else {
                                "NFO not exported"
                            });
                        }
                    }
                    3 => {
                        ui.heading(if video.artwork.thumb.is_some() {
                            "Episode thumbnail"
                        } else {
                            "Poster"
                        });
                        if !matching::video_poster(
                            ui,
                            &video,
                            false,
                            if video.artwork.thumb.is_some() {
                                egui::vec2(
                                    480.0_f32.min(ui.available_width()),
                                    270.0_f32.min(ui.available_width() * 9.0 / 16.0),
                                )
                            } else {
                                egui::vec2(220.0, 330.0)
                            },
                        ) {
                            ui.weak("No poster found.");
                        }
                        ui.add_space(20.0);
                        ui.heading("Fanart");
                        let w = ui.available_width().min(640.0);
                        if !matching::video_poster(ui, &video, true, egui::vec2(w, w * 9.0 / 16.0))
                        {
                            ui.weak("No fanart found.");
                        }
                    }
                    4 => {
                        ui.heading("Trailer");
                        ui.add_space(12.0);
                        if let Some(url) = data.and_then(|m| m.trailer.as_ref()) {
                            ui.hyperlink_to("Watch trailer", url);
                        } else {
                            ui.weak("No trailer available.");
                        }
                    }
                    _ => {
                        let poster_width = (ui.available_width() * 0.30).clamp(150.0, 235.0);
                        let poster_size = egui::vec2(
                            poster_width,
                            if video.artwork.thumb.is_some() {
                                poster_width * 9.0 / 16.0
                            } else {
                                poster_width * 1.5
                            },
                        );
                        ui.horizontal_top(|ui| {
                            ui.allocate_ui_with_layout(
                                poster_size,
                                egui::Layout::top_down(egui::Align::Min),
                                |ui| {
                                    if !matching::video_poster(ui, &video, false, poster_size) {
                                        egui::Frame::new()
                                            .fill(theme::CARD)
                                            .corner_radius(10)
                                            .show(ui, |ui| {
                                                ui.allocate_ui_with_layout(
                                                    egui::vec2(poster_width, poster_width * 1.5),
                                                    egui::Layout::centered_and_justified(
                                                        egui::Direction::TopDown,
                                                    ),
                                                    |ui| {
                                                        ui.weak("No artwork yet");
                                                    },
                                                );
                                            });
                                    }
                                },
                            );
                            ui.add_space(12.0);
                            ui.vertical(|ui| {
                                ui.set_width(ui.available_width());
                                ui.add(
                                    egui::Label::new(
                                        RichText::new(title(&video)).size(32.0).strong(),
                                    )
                                    .wrap(),
                                );
                                if let Some(m) = &video.tmdb {
                                    if m.original_title != m.title && !m.original_title.is_empty() {
                                        ui.weak(&m.original_title);
                                    }
                                }
                                ui.add_space(6.0);
                                ui.horizontal_wrapped(|ui| {
                                    let y = year(&video);
                                    if !y.is_empty() {
                                        theme::badge(ui, &y);
                                    }
                                    if let Some(data) = data {
                                        if let Some(r) = data.runtime {
                                            theme::badge(ui, &format!("{}h {}m", r / 60, r % 60));
                                        }
                                        if let Some(cert) = &data.certification {
                                            if !cert.is_empty() {
                                                theme::badge(ui, cert);
                                            }
                                        }
                                        if let Some(r) = data.rating {
                                            ui.label(
                                                RichText::new(format!("{r:.1} / 10"))
                                                    .color(theme::ACCENT)
                                                    .strong(),
                                            )
                                            .on_hover_text(format!(
                                                "{} votes",
                                                data.votes.unwrap_or(0)
                                            ));
                                        }
                                    }
                                });
                                ui.add_space(10.0);
                                if let Some(data) = data {
                                    if !data.genres.is_empty() {
                                        ui.add(
                                            egui::Label::new(
                                                RichText::new(data.genres.join("  ·  "))
                                                    .color(theme::MUTED),
                                            )
                                            .wrap(),
                                        );
                                    }
                                    if let Some(tagline) = &data.tagline {
                                        if !tagline.is_empty() {
                                            ui.add_space(12.0);
                                            ui.add(
                                                egui::Label::new(
                                                    RichText::new(tagline)
                                                        .italics()
                                                        .color(theme::MUTED),
                                                )
                                                .wrap(),
                                            );
                                        }
                                    }
                                }
                                ui.add_space(16.0);
                                let locked = self.item_actions_locked();
                                ui.horizontal_wrapped(|ui| {
                                    if ui
                                        .add_enabled(
                                            !locked,
                                            egui::Button::new("Search & scrape")
                                                .fill(egui::Color32::from_rgb(222, 233, 250)),
                                        )
                                        .clicked()
                                    {
                                        self.tmdb_ui.open_many(self.selected_videos(false), false);
                                    }
                                    if ui
                                        .add_enabled(!locked, egui::Button::new("Rename"))
                                        .clicked()
                                    {
                                        self.apply_item_context(
                                            vec![video.path.clone()],
                                            Some(ItemAction::Rename),
                                        );
                                    }
                                });
                            });
                        });
                        ui.add_space(28.0);
                        ui.label(RichText::new("Overview").size(18.0).strong());
                        ui.add_space(4.0);
                        if let Some(data) = data {
                            ui.add(egui::Label::new(&data.identity.overview).wrap());
                            ui.add_space(24.0);
                            egui::Frame::new()
                                .fill(theme::PANEL)
                                .corner_radius(10)
                                .inner_margin(18)
                                .show(ui, |ui| {
                                    ui.set_width((ui.available_width()).max(100.0));
                                    egui::Grid::new("metadata_details")
                                        .num_columns(2)
                                        .spacing([24.0, 12.0])
                                        .show(ui, |ui| {
                                            for (label, value) in [
                                                (
                                                    "Released",
                                                    data.identity
                                                        .release_date
                                                        .clone()
                                                        .unwrap_or_default(),
                                                ),
                                                ("Country", data.countries.join(", ")),
                                                ("Languages", data.languages.join(", ")),
                                                ("Studio", data.studios.join(" / ")),
                                                (
                                                    "Collection",
                                                    data.collection.clone().unwrap_or_default(),
                                                ),
                                            ] {
                                                if !value.is_empty() {
                                                    ui.weak(label);
                                                    ui.add_sized(
                                                        [ui.available_width().max(120.0), 0.0],
                                                        egui::Label::new(value).wrap(),
                                                    );
                                                    ui.end_row();
                                                }
                                            }
                                            ui.weak("TMDB");
                                            ui.hyperlink_to(
                                                data.identity.id.to_string(),
                                                data.identity.url(),
                                            );
                                            ui.end_row();
                                            if let Some(id) = &data.imdb_id {
                                                ui.weak("IMDb");
                                                ui.label(id);
                                                ui.end_row();
                                            }
                                        });
                                });
                        } else {
                            ui.weak(
                                "Search for a match to add the plot, artwork and other details.",
                            );
                            if let Some(m) = &video.tmdb {
                                ui.add(egui::Label::new(&m.overview).wrap());
                            }
                        }
                        if let Some(data) = data {
                            if !data.tags.is_empty() {
                                ui.add_space(12.0);
                                egui::CollapsingHeader::new("Tags").show(ui, |ui| {
                                    ui.add(egui::Label::new(data.tags.join(", ")).wrap());
                                });
                            }
                        }
                        ui.add_space(20.0);
                        if let Some(folder) = if video.is_folder {
                            Some(video.path.as_path())
                        } else {
                            video.path.parent()
                        } {
                            if let Some(error) = folder_link(ui, &display_path(&video.path), folder)
                            {
                                self.error = Some(error);
                            }
                        }
                    }
                }
            });
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _: &mut eframe::Frame) {
        self.poll();
        self.series_tree.poll(ctx);
        let scanning = self.receiver.is_some();
        let locked = scanning || self.file_tools.active() || self.tmdb_ui.active();
        if scanning {
            ctx.request_repaint_after(Duration::from_millis(100));
        }
        egui::TopBottomPanel::top("toolbar")
            .frame(
                egui::Frame::new()
                    .fill(theme::PANEL)
                    .inner_margin(egui::Margin::symmetric(20, 14)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new("media shelf")
                            .size(22.0)
                            .strong()
                            .color(theme::TEXT),
                    );
                    ui.separator();
                    if scanning {
                        if ui.button("Cancel update").clicked() {
                            self.cancel.store(true, Ordering::Relaxed);
                        }
                        ui.spinner();
                    } else if ui
                        .add_enabled(
                            !locked && !self.sources.is_empty(),
                            egui::Button::new("Update sources"),
                        )
                        .clicked()
                    {
                        self.start_scan();
                    }
                    let targets = self.selected_videos(false);
                    if ui
                        .add_enabled(
                            !locked && !targets.is_empty(),
                            egui::Button::new("Search & scrape"),
                        )
                        .clicked()
                    {
                        self.tmdb_ui.open_many(targets.clone(), false);
                    }
                    ui.add_enabled_ui(!locked, |ui| {
                        ui.menu_button("▾", |ui| {
                            if ui
                                .add_enabled(
                                    !targets.is_empty(),
                                    egui::Button::new("Search & scrape selected"),
                                )
                                .clicked()
                            {
                                self.tmdb_ui.open_many(targets.clone(), false);
                                ui.close();
                            }
                            let known: Vec<_> = targets
                                .iter()
                                .filter(|v| v.tmdb.is_some())
                                .cloned()
                                .collect();
                            if ui
                                .add_enabled(
                                    !known.is_empty(),
                                    egui::Button::new("Re-scrape using saved IDs"),
                                )
                                .clicked()
                            {
                                self.tmdb_ui.open_many(known, true);
                                ui.close();
                            }
                            if ui.button("Search & scrape unscraped items").clicked() {
                                self.tmdb_ui.open_many(self.selected_videos(true), false);
                                ui.close();
                            }
                        });
                        self.organize_menu(ui);
                    });
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("Settings").clicked() {
                            self.tmdb_ui.settings_open = true;
                        }
                        if ui
                            .add_enabled(!locked, egui::Button::new("Data sources"))
                            .clicked()
                        {
                            self.sources_open = true;
                        }
                    });
                });
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    for (category, label) in
                        [(Category::Movies, "Movies"), (Category::Series, "TV shows")]
                    {
                        if ui
                            .selectable_label(self.category == category, label)
                            .clicked()
                        {
                            self.category = category;
                            self.sort = SortColumn::Title;
                            self.sort_descending = false;
                            self.selection.clear();
                            self.selected = None;
                            self.selected_season = None;
                        }
                    }
                });
            });
        egui::TopBottomPanel::bottom("status").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.small(&self.status);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.weak("Media Shelf 0.6");
                });
            });
        });
        egui::SidePanel::left("library")
            .default_width(440.0)
            .min_width(420.0)
            .max_width(600.0)
            .frame(egui::Frame::new().fill(theme::PANEL).inner_margin(18))
            .resizable(true)
            .show(ctx, |ui| {
                self.library_list(ui);
            });
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(theme::BG).inner_margin(24))
            .show(ctx, |ui| {
                if let Some(error) = self.error.clone() {
                    ui.horizontal_wrapped(|ui| {
                        ui.colored_label(Color32::from_rgb(170, 35, 35), error);
                        if ui.small_button("Dismiss").clicked() {
                            self.error = None;
                        }
                    });
                }
                if !self.warnings.is_empty() {
                    egui::CollapsingHeader::new(format!("{} scan warnings", self.warnings.len()))
                        .show(ui, |ui| {
                            egui::ScrollArea::vertical()
                                .max_height(100.0)
                                .show(ui, |ui| {
                                    for w in &self.warnings {
                                        ui.small(w);
                                    }
                                });
                        });
                }
                self.detail_view(ui);
            });
        self.sources_window(ctx, locked);
        let missing_before = self.tmdb_ui.show_missing_episodes;
        let scrape_changed = self.tmdb_ui.draw(
            ctx,
            self.database.as_mut(),
            scanning || self.file_tools.active(),
        );
        if missing_before != self.tmdb_ui.show_missing_episodes {
            self.series_tree = Default::default();
            ctx.request_repaint();
        }
        let files_changed = self.file_tools.draw(ctx);
        if files_changed {
            self.selected = None;
            self.selected_season = None;
            self.selection.clear();
        }
        if (scrape_changed || files_changed)
            && let Some(db) = &self.database
        {
            match db.videos() {
                Ok(videos) => {
                    self.videos = videos;
                    self.series_tree = Default::default();
                    self.status = if scrape_changed {
                        "Scrape complete · Selected metadata saved"
                    } else {
                        "File operation finished · Library refreshed"
                    }
                    .into();
                }
                Err(e) => self.error = Some(e.to_string()),
            }
        }
    }
}
