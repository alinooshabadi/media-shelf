use super::*;
use std::{collections::BTreeMap, path::Path};

impl App {
    pub(super) fn select_season(&mut self, folder: PathBuf, season: u32) {
        self.selection.clear();
        self.selection.insert(folder.clone());
        self.selected = Some(folder.clone());
        self.selected_season = Some((folder, season));
    }

    pub(super) fn season_view(&mut self, ui: &mut egui::Ui, folder: &Path, season: u32) {
        let name = self
            .videos
            .iter()
            .find(|v| v.path == folder)
            .map(super::library_view::title)
            .unwrap_or_else(|| {
                folder
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into_owned()
            });
        ui.heading(name);
        ui.label(if season == 0 {
            "Specials".into()
        } else {
            format!("Season {season}")
        });
        ui.separator();
        let Some(show) = self.series_tree.shows.get(folder) else {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label("Loading episodes…");
            });
            return;
        };
        if show.checking_metadata {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.weak("Loading episode details…");
            });
        }
        if !show.warnings.is_empty() {
            ui.colored_label(
                Color32::from_rgb(170, 85, 55),
                "Some episode details are unavailable.",
            )
            .on_hover_text(show.warnings.join("\n"));
        }
        let mut rows = vec![];
        if let Some(episodes) = show.seasons.get(&Some(season)) {
            for e in episodes {
                let title = e.details.title.clone();
                rows.push((
                    e.number.unwrap_or(u32::MAX),
                    if title.is_empty() {
                        e.label.clone()
                    } else {
                        title
                    },
                    e.details.aired.clone(),
                    e.details.rating,
                    Some(e.video.path.clone()),
                ));
            }
        }
        if self.tmdb_ui.show_missing_episodes {
            if let Some(missing) = show.missing.get(&season) {
                let empty = BTreeMap::new();
                let details = show.metadata.get(&season).unwrap_or(&empty);
                for (number, title) in missing {
                    let info = details.get(number);
                    rows.push((
                        *number,
                        format!("{title} · Missing"),
                        info.and_then(|e| e.aired.clone()),
                        info.and_then(|e| e.rating),
                        None,
                    ));
                }
            }
        }
        rows.sort_by_key(|(n, _, _, _, _)| *n);
        ui.add_space(8.0);
        ui.strong(format!("Episodes ({})", rows.len()));
        if rows.is_empty() {
            ui.weak("No episodes in this season.");
            return;
        }
        let mut click = None;
        egui_extras::TableBuilder::new(ui)
            .id_salt(("season_episodes", folder, season))
            .striped(true)
            .resizable(false)
            .sense(egui::Sense::click())
            .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
            .column(egui_extras::Column::exact(64.0))
            .column(egui_extras::Column::remainder().at_least(100.0))
            .column(egui_extras::Column::exact(100.0))
            .column(egui_extras::Column::exact(60.0))
            .header(28.0, |mut row| {
                for label in ["Episode", "Title", "Aired", "Rating"] {
                    row.col(|ui| {
                        ui.strong(label);
                    });
                }
            })
            .body(|body| {
                body.rows(28.0, rows.len(), |mut row| {
                    let (number, title, aired, rating, path) = &rows[row.index()];
                    row.col(|ui| {
                        ui.label(if *number == u32::MAX {
                            "–".into()
                        } else {
                            number.to_string()
                        });
                    });
                    row.col(|ui| {
                        let label = egui::Label::new(if path.is_none() {
                            RichText::new(title).color(Color32::from_rgb(170, 85, 55))
                        } else {
                            RichText::new(title)
                        })
                        .truncate()
                        .sense(egui::Sense::click());
                        let response = ui.add(label).on_hover_text(path.as_ref().map_or_else(
                            || "Not in the local library".into(),
                            |p| p.display().to_string(),
                        ));
                        if response.clicked() {
                            click = path.clone();
                        }
                    });
                    row.col(|ui| {
                        ui.label(aired.as_deref().unwrap_or("–"));
                    });
                    row.col(|ui| {
                        ui.label(
                            rating
                                .map(|r| format!("{r:.1}"))
                                .unwrap_or_else(|| "–".into()),
                        );
                    });
                    if row.response().clicked() {
                        click = path.clone();
                    }
                });
            });
        if let Some(path) = click {
            self.select(path, false);
            self.detail_tab = 2;
        }
    }
}
