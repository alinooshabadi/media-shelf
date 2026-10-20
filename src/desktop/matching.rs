use eframe::egui::{self, RichText};
use media_shelf::{
    Category, Video,
    metadata::{self, Field, Metadata, ScrapeOptions},
    storage::{LibraryDb, database_path},
    tmdb::{self, SearchPage},
};
use std::{collections::VecDeque, sync::mpsc};
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn movie_and_show_chooser_render_without_committing() {
        for (category, height) in [
            (Category::Movies, 880.0),
            (Category::Series, 880.0),
            (Category::Movies, 600.0),
        ] {
            let identity = tmdb::TmdbMatch {
                category,
                id: 42,
                title: "Example title".into(),
                original_title: "Original title".into(),
                release_date: Some("2024-01-01".into()),
                overview: "Example plot".into(),
            };
            let video = Video {
                artwork: Default::default(),
                category,
                path: "C:/fixtures/Example.mkv".into(),
                relative_path: "Example.mkv".into(),
                bytes: 1024,
                tmdb: None,
                metadata: None,
            };
            let mut dialog = SearchDialog::new(video);
            dialog.results = vec![identity.clone()];
            dialog.picked = Some(0);
            dialog.details = Some(Metadata::empty(identity));
            let mut chooser = TmdbUi::new();
            chooser.token.clear();
            chooser.dialog = Some(dialog);
            let ctx = egui::Context::default();
            crate::desktop::theme::install(&ctx);
            for frame in 0..3 {
                let output = ctx.run(
                    egui::RawInput {
                        screen_rect: Some(egui::Rect::from_min_size(
                            egui::Pos2::ZERO,
                            egui::vec2(1380.0, height),
                        )),
                        ..Default::default()
                    },
                    |ctx| {
                        assert!(!chooser.draw(ctx, None, false));
                    },
                );
                assert!(!output.shapes.is_empty());
                if frame == 2 {
                    let mut apply_visible = false;
                    let mut apply_rect = None;
                    for clipped in &output.shapes {
                        if let egui::Shape::Text(text) = &clipped.shape {
                            if text.galley.job.text == "OK — Scrape" {
                                let rect = text.galley.rect.translate(text.pos.to_vec2());
                                apply_rect = Some((rect, clipped.clip_rect));
                                apply_visible = clipped.clip_rect.contains_rect(rect)
                                    && ctx.content_rect().contains_rect(rect);
                            }
                        }
                    }
                    assert!(
                        apply_visible,
                        "Apply button must remain visible: height={height}, rect={apply_rect:?}"
                    );
                }
            }
            let dialog = chooser.dialog.as_ref().unwrap();
            assert!(dialog.video.metadata.is_none());
            assert!(dialog.save_rx.is_none());
            assert_eq!(dialog.whole_show, category == Category::Series);
            assert_eq!(dialog.details.as_ref().unwrap().identity.id, 42);
        }
    }
}
pub const LANGUAGES: &[(&str, &str)] = &[
    ("en-US", "English"),
    ("de-DE", "Deutsch"),
    ("fr-FR", "Français"),
    ("es-ES", "Español"),
    ("it-IT", "Italiano"),
    ("pt-BR", "Português"),
    ("ja-JP", "日本語"),
    ("ko-KR", "한국어"),
    ("zh-CN", "中文"),
];
pub struct TmdbUi {
    token: String,
    token_status: Option<String>,
    pub settings_open: bool,
    language: String,
    options: ScrapeOptions,
    dialog: Option<SearchDialog>,
    queue: VecDeque<Video>,
    total: usize,
    known_id: bool,
    pub notice: Option<String>,
}
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
fn suggested_query(video: &Video) -> String {
    if let Some(m) = &video.tmdb {
        return m.title.clone();
    }
    if video.category == Category::Movies {
        return media_shelf::identification::local_identity(video).0;
    }
    let name = video
        .path
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy()
        .replace(['.', '_'], " ");
    let stop=regex::Regex::new(r"(?i)\s*(?:\(?\b(?:19|20)\d{2}\b\)?|\bS\d{1,3}E\d{1,3}\b|\b\d{3,4}p\b|\b(?:bluray|web-dl|webrip|x264|x265|h264|h265)\b).*$").unwrap();
    let cleaned = stop.replace(&name, "").trim().to_string();
    if cleaned.is_empty() { name } else { cleaned }
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
            let _ = tx.send(tmdb::search_language(
                category, &query, &token, page, &language,
            ));
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
impl TmdbUi {
    pub fn new() -> Self {
        let (saved_token, token_status) =
            match media_shelf::credentials::load(&media_shelf::credentials::token_path()) {
                Ok(token) => (token, None),
                Err(_) => (
                    String::new(),
                    Some(
                        "Could not read the saved token. Enter it again and click Save token."
                            .into(),
                    ),
                ),
            };
        Self {
            token: if saved_token.is_empty() {
                std::env::var("TMDB_READ_ACCESS_TOKEN").unwrap_or_default()
            } else {
                saved_token
            },
            token_status,
            settings_open: false,
            language: "en-US".into(),
            options: ScrapeOptions::default(),
            dialog: None,
            queue: VecDeque::new(),
            total: 0,
            known_id: false,
            notice: None,
        }
    }
    pub fn active(&self) -> bool {
        self.dialog.is_some()
    }
    pub fn open_many(&mut self, videos: Vec<Video>, known_id: bool) {
        self.queue = videos.into();
        self.total = self.queue.len();
        self.known_id = known_id;
        self.notice = None;
        self.next(known_id);
    }
    fn next(&mut self, known_id: bool) {
        self.dialog = self.queue.pop_front().map(SearchDialog::new);
        if let Some(dialog) = &mut self.dialog {
            if self.token.trim().is_empty() {
                self.settings_open = true;
            } else if known_id && let Some(m) = dialog.video.tmdb.clone() {
                dialog.results = vec![m];
                dialog.preview(0, &self.token, &self.language);
            } else {
                dialog.search(&self.token, &self.language, 1);
            }
        }
    }
    pub fn draw(&mut self, ctx: &egui::Context, _db: Option<&mut LibraryDb>, locked: bool) -> bool {
        self.settings(ctx);
        let Some(dialog) = &mut self.dialog else {
            return false;
        };
        let mut reload = false;
        let mut advance = false;
        if let Some(rx) = &dialog.save_rx {
            match rx.try_recv() {
                Ok(result) => {
                    dialog.save_rx = None;
                    match result {
                        Ok((count, warnings)) => {
                            reload = true;
                            advance = true;
                            if !warnings.is_empty() {
                                self.notice = Some(format!(
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
                    reload = true;
                }
                Err(mpsc::TryRecvError::Empty) => {}
            }
        }
        if advance {
            self.next(self.known_id);
            return reload;
        }
        if let Some(rx) = &dialog.search_rx {
            match rx.try_recv() {
                Ok(result) => {
                    dialog.search_rx = None;
                    match result {
                        Ok(page) => {
                            dialog.results = page.results;
                            dialog.page = page.page;
                            dialog.total_pages = page.total_pages;
                            if !dialog.results.is_empty() {
                                dialog.preview(0, &self.token, &self.language);
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
        let saving = dialog.save_rx.is_some();
        let searching = dialog.search_rx.is_some();
        let loading = dialog.detail_rx.is_some();
        if saving || searching || loading {
            ctx.request_repaint_after(std::time::Duration::from_millis(80));
        }
        let mut open = true;
        let mut search = None;
        let mut pick = None;
        let mut confirm = false;
        let mut skip = false;
        let mut cancel = false;
        let mut window = egui::Window::new(if dialog.video.category == Category::Movies {
            "Search movie"
        } else {
            "Search TV show"
        })
        .id(egui::Id::new("search_scrape"))
        .default_width(1040.0)
        .default_height(700.0)
        .min_width(820.0)
        .resizable(true)
        .collapsible(false);
        if !saving {
            window = window.open(&mut open);
        }
        window.show(ctx,|ui|{
            let body_height = (ui.available_height() - 64.0).max(80.0);
            egui::ScrollArea::vertical().id_salt("scrape_body").max_height(body_height).auto_shrink([false, false]).show(ui, |ui| {
            ui.add(egui::Label::new(RichText::new(dialog.video.path.display().to_string()).strong()).truncate()).on_hover_text(dialog.video.path.display().to_string());
            if self.total>1{ui.label(format!("Item {} of {}",self.total-self.queue.len(),self.total));}ui.separator();
            ui.add_enabled_ui(!saving&&!searching,|ui|{
                ui.horizontal(|ui|{
                    ui.label("Scraper");egui::ComboBox::from_id_salt("scraper").selected_text("themoviedb.org").show_ui(ui,|ui|{ui.label("themoviedb.org");});
                    let response=ui.add(egui::TextEdit::singleline(&mut dialog.query).desired_width((ui.available_width()-100.0).max(200.0)).char_limit(200));
                    if ui.button("Search").clicked()||(response.lost_focus()&&ui.input(|i|i.key_pressed(egui::Key::Enter))){search=Some(1);}
                });
                ui.horizontal(|ui|{
                    ui.label("Language");let previous=self.language.clone();
                    egui::ComboBox::from_id_salt("scrape_language").selected_text(LANGUAGES.iter().find(|(code,_)|*code==self.language).map(|(_,name)|*name).unwrap_or("English")).show_ui(ui,|ui|{for(code,name)in LANGUAGES{ui.selectable_value(&mut self.language,code.to_string(),*name);}});
                    if previous!=self.language{search=Some(1);}if ui.small_button("API settings").clicked(){self.settings_open=true;}
                });
            });ui.separator();
            ui.horizontal_top(|ui|{
                ui.allocate_ui_with_layout(egui::vec2(265.0,330.0),egui::Layout::top_down(egui::Align::Min),|ui|{
                    ui.horizontal(|ui|{ui.strong("Search result");ui.with_layout(egui::Layout::right_to_left(egui::Align::Center),|ui|{ui.strong("Year");});});
                    egui::ScrollArea::vertical().id_salt("candidate_list").max_height(280.0).show(ui,|ui|{
                        for(index,result)in dialog.results.iter().enumerate(){if ui.add_sized([250.0,23.0],egui::Button::selectable(dialog.picked==Some(index),format!("{}  ({})",result.title,result.year())).truncate()).clicked()&&!saving{pick=Some(index);}}
                        if dialog.searched&&dialog.results.is_empty()&&!searching{ui.label("No results. Edit the title and search again.");}
                    });
                    ui.horizontal(|ui|{
                        if ui.add_enabled(!searching&&!saving&&dialog.page>1,egui::Button::new("‹")).clicked(){search=Some(dialog.page-1);}ui.label(format!("{} / {}",dialog.page,dialog.total_pages.max(1)));
                        if ui.add_enabled(!searching&&!saving&&dialog.page<dialog.total_pages,egui::Button::new("›")).clicked(){search=Some(dialog.page+1);}
                    });
                });ui.add_sized([8.0, 360.0], egui::Separator::default().vertical());
                ui.vertical(|ui|{
                    if searching||loading{ui.horizontal(|ui|{ui.spinner();ui.label(if searching{"Searching…"}else{"Loading candidate details…"});});}
                    if let Some(data)=&dialog.details{
                        ui.horizontal_top(|ui|{
                            ui.allocate_ui_with_layout(egui::vec2(145.0,210.0),egui::Layout::top_down(egui::Align::Min),|ui|{poster(ui,data,false,egui::vec2(140.0,205.0));});
                            ui.vertical(|ui|{
                                ui.heading(format!("{} ({})",data.identity.title,data.year.map(|y|y.to_string()).unwrap_or_else(||"—".into())));ui.label(&data.identity.original_title);
                                if let Some(tagline)=&data.tagline{ui.label(RichText::new(tagline).italics());}
                                egui::ScrollArea::vertical().id_salt("candidate_plot").max_height(155.0).show(ui,|ui|{ui.label(&data.identity.overview);});
                            });
                        });ui.separator();
                        egui::ScrollArea::vertical().id_salt("candidate_people").max_height(140.0).show(ui,|ui|{
                            egui::Grid::new("people_preview").num_columns(2).striped(true).show(ui,|ui|{
                                ui.strong("Name");ui.strong("Role");ui.end_row();for p in data.crew.iter().filter(|p|matches!(p.role.as_str(),"Director"|"Producer"|"Executive Producer"|"Writer"|"Screenplay"|"Creator")).chain(data.cast.iter()).take(100){ui.label(&p.name);ui.label(&p.role);ui.end_row();}
                            });
                        });
                    }else if !searching&&!loading{ui.label("Select a candidate to preview its details.");}
                });
            });
            if let Some(error)=&dialog.error{ui.colored_label(egui::Color32::from_rgb(175,40,35),error);}ui.separator();ui.strong("Scrape following items");
            ui.add_enabled_ui(!saving,|ui|{
                ui.horizontal_wrapped(|ui|{for field in Field::ALL{if *field==Field::MovieSet&&dialog.video.category==Category::Series{continue;}let mut chosen=self.options.fields.contains(field);if ui.checkbox(&mut chosen,field.label()).changed(){if chosen{self.options.fields.push(*field);}else{self.options.fields.retain(|f|f!=field);}}}});
                ui.horizontal_wrapped(|ui|{if ui.small_button("Select all").clicked(){self.options.fields=Field::ALL.to_vec();}if ui.small_button("Select none").clicked(){self.options.fields.clear();}ui.checkbox(&mut self.options.only_missing,"Fill missing fields only");ui.checkbox(&mut self.options.write_nfo,"Write NFO");ui.checkbox(&mut self.options.replace_nfo,"Replace existing NFO/artwork (backup first)");});
                if dialog.video.category==Category::Series{ui.label(RichText::new("Applies to this TV show and its files. Individual episode titles and plots are not scraped yet.").small().weak());}
            });ui.separator();
            });
            ui.horizontal(|ui|{
                if saving{ui.spinner();ui.label("Scraping selected fields and writing outputs…");}
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center),|ui|{
                    if ui.add_enabled(!saving&&!searching&&!loading&&!locked&&dialog.details.is_some()&&!self.options.fields.is_empty(),egui::Button::new("OK — Scrape")).clicked(){confirm=true;}
                    if ui.add_enabled(!saving,egui::Button::new("Cancel")).clicked(){cancel=true;}if self.total>1&&ui.add_enabled(!saving,egui::Button::new("Skip")).clicked(){skip=true;}
                });
            });
        });
        if let Some(page) = search {
            dialog.search(&self.token, &self.language, page);
        } else if let Some(index) = pick {
            dialog.preview(index, &self.token, &self.language);
        } else if confirm && let Some(data) = dialog.details.clone() {
            let video = dialog.video.clone();
            let options = self.options.clone();
            let group = dialog.whole_show;
            let (tx, rx) = mpsc::channel();
            dialog.save_rx = Some(rx);
            dialog.error = None;
            std::thread::spawn(move || {
                let result = (|| -> media_shelf::storage::DbResult<_> {
                    let mut db = LibraryDb::open(&database_path())?;
                    metadata::commit_scrape(&mut db, &video, &data, &options, group)
                })();
                let _ = tx.send(result.map_err(|e| e.to_string()));
            });
        }
        if skip {
            self.next(self.known_id);
        } else if cancel || !open {
            self.dialog = None;
            self.queue.clear();
        }
        reload
    }
    fn settings(&mut self, ctx: &egui::Context) {
        if self.settings_open {
            egui::Window::new("TMDB settings & credits").open(&mut self.settings_open).default_width(500.0).show(ctx,|ui|{
            ui.label("TMDB API Read Access Token");ui.add(egui::TextEdit::singleline(&mut self.token).password(true).desired_width(f32::INFINITY));ui.horizontal(|ui| {
    if ui.button("Save token").clicked() {
        self.token_status = Some(match media_shelf::credentials::save(&media_shelf::credentials::token_path(), &self.token) {
            Ok(()) => if self.token.trim().is_empty() { "Saved token removed.".into() } else { "Token saved. It will load automatically next time.".into() },
            Err(_) => "Could not save the token. You can still use it for this session.".into(),
        });
    }
    if ui.button("Forget token").clicked() {
        self.token_status = Some(match media_shelf::credentials::save(&media_shelf::credentials::token_path(), "") {
            Ok(()) => { self.token.clear(); "Saved token removed.".into() },
            Err(_) => "Could not remove the saved token.".into(),
        });
    }
});
if let Some(status) = &self.token_status { ui.label(status); }
ui.label("Saved in your user profile. TMDB_READ_ACCESS_TOKEN is used when no saved token exists.");ui.hyperlink_to("TMDB account API settings","https://www.themoviedb.org/settings/api");ui.separator();ui.add(egui::Image::from_bytes("bytes://tmdb-logo.svg",include_bytes!("../../assets/tmdb-logo.svg")).fit_to_exact_size(egui::vec2(92.52,66.7)));ui.label("This product uses the TMDB API but is not endorsed or certified by TMDB.");ui.hyperlink_to("The Movie Database","https://www.themoviedb.org");
        });
        }
        if self.dialog.is_none()
            && let Some(message) = self.notice.clone()
        {
            let mut open = true;
            egui::Window::new("Scrape report")
                .open(&mut open)
                .default_width(650.0)
                .show(ctx, |ui| {
                    egui::ScrollArea::vertical()
                        .max_height(300.0)
                        .show(ui, |ui| {
                            ui.label(message);
                        });
                });
            if !open {
                self.notice = None;
            }
        }
    }
}
fn local_image_uri(path: &std::path::Path) -> String {
    let path = path.to_string_lossy();
    let path = if let Some(unc) = path.strip_prefix("\\\\?\\UNC\\") {
        format!("//{}", unc.replace('\\', "/"))
    } else {
        path.strip_prefix("\\\\?\\")
            .unwrap_or(&path)
            .replace('\\', "/")
    };
    format!("file://{path}")
}

pub fn video_poster(ui: &mut egui::Ui, video: &Video, fanart: bool, size: egui::Vec2) -> bool {
    let local = if fanart {
        video.artwork.fanart.as_ref()
    } else {
        video.artwork.poster.as_ref()
    };
    if let Some(path) = local {
        ui.add(
            egui::Image::new(local_image_uri(path))
                .fit_to_exact_size(size)
                .corner_radius(8),
        );
        return true;
    }
    if let Some(data) = &video.metadata {
        let present = if fanart {
            data.fanart_file.is_some() || data.fanart.is_some()
        } else {
            data.poster_file.is_some() || data.poster.is_some()
        };
        if present {
            poster(ui, data, fanart, size);
            return true;
        }
    }
    false
}

pub fn poster(ui: &mut egui::Ui, data: &Metadata, fanart: bool, size: egui::Vec2) {
    let local = if fanart {
        data.fanart_file.as_ref()
    } else {
        data.poster_file.as_ref()
    };
    let remote = if fanart {
        data.fanart.as_deref()
    } else {
        data.poster.as_deref()
    };
    if let Some(local) = local {
        ui.add(
            egui::Image::new(local_image_uri(local))
                .fit_to_exact_size(size)
                .corner_radius(8),
        );
    } else if let Some(url) = remote.and_then(|p| metadata::image_url(p, fanart)) {
        ui.add(
            egui::Image::new(url)
                .fit_to_exact_size(size)
                .corner_radius(8),
        );
    } else {
        ui.allocate_ui_with_layout(
            size,
            egui::Layout::centered_and_justified(egui::Direction::TopDown),
            |ui| {
                ui.weak(if fanart { "No fanart" } else { "No poster" });
            },
        );
    }
}
