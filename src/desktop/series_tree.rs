//! Episode browsing is loaded on demand; the saved library stays a shallow show index.
use super::*;
use media_shelf::metadata::EpisodeDetails;
use std::collections::{BTreeMap, HashMap};
use std::path::Path;

#[derive(Clone)]
pub(super) struct Episode {
    pub video: Video,
    pub number: Option<u32>,
    pub needs_title: bool,
    pub label: String,
    pub details: EpisodeDetails,
}

#[derive(Clone, Default)]
pub(super) struct Show {
    pub seasons: BTreeMap<Option<u32>, Vec<Episode>>,
    pub warnings: Vec<String>,
    pub missing: BTreeMap<u32, BTreeMap<u32, String>>,
    pub checking_metadata: bool,
    pub metadata: BTreeMap<u32, BTreeMap<u32, EpisodeDetails>>,
    present: std::collections::BTreeSet<(u32, u32)>,
    unrecognized: usize,
}
impl Show {
    pub fn season_count(&self) -> usize {
        self.seasons
            .iter()
            .filter(|(n, e)| n.is_some() && !e.is_empty())
            .count()
    }
    pub fn add_missing(&mut self, catalog: BTreeMap<u32, BTreeMap<u32, String>>) {
        for (season, episodes) in catalog {
            let details = episodes
                .iter()
                .map(|(n, t)| {
                    (
                        *n,
                        EpisodeDetails {
                            title: t.clone(),
                            ..Default::default()
                        },
                    )
                })
                .collect();
            self.fill_details(season, &details);
            for (number, title) in episodes {
                if !self.present.contains(&(season, number)) {
                    self.missing
                        .entry(season)
                        .or_default()
                        .insert(number, title);
                    self.seasons.entry(Some(season)).or_default();
                }
            }
        }
    }
    fn fill_details(&mut self, season: u32, details: &BTreeMap<u32, EpisodeDetails>) {
        if let Some(episodes) = self.seasons.get_mut(&Some(season)) {
            for episode in episodes {
                if let Some(info) = episode.number.and_then(|n| details.get(&n)) {
                    if episode.needs_title && !info.title.trim().is_empty() {
                        episode.label = format!("{}. {}", episode.number.unwrap(), info.title);
                        episode.details.title = info.title.clone();
                        episode.needs_title = false;
                    }
                    if episode.details.aired.is_none() {
                        episode.details.aired = info.aired.clone();
                    }
                    if episode.details.rating.is_none() {
                        episode.details.rating = info.rating;
                    }
                }
            }
        }
        self.metadata.insert(season, details.clone());
    }

    fn apply_catalog(&mut self, catalog: BTreeMap<u32, BTreeMap<u32, EpisodeDetails>>) {
        self.add_missing(
            catalog
                .iter()
                .map(|(s, episodes)| {
                    (
                        *s,
                        episodes
                            .iter()
                            .map(|(n, e)| (*n, e.title.clone()))
                            .collect(),
                    )
                })
                .collect(),
        );
        for (season, details) in catalog {
            self.fill_details(season, &details);
        }
    }

    fn incomplete_seasons(&self) -> Vec<u32> {
        self.seasons
            .iter()
            .filter_map(|(season, episodes)| {
                season.filter(|_| {
                    episodes.iter().any(|e| {
                        e.needs_title || e.details.aired.is_none() || e.details.rating.is_none()
                    })
                })
            })
            .collect()
    }

    pub fn count(&self) -> usize {
        self.seasons.values().map(Vec::len).sum()
    }
}

#[derive(Default)]
pub(super) struct SeriesTree {
    pub shows: HashMap<PathBuf, Show>,
    pending: HashMap<PathBuf, mpsc::Receiver<Show>>,
    cancel: Arc<AtomicBool>,
}
impl Drop for SeriesTree {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
    }
}
impl SeriesTree {
    pub fn request(
        &mut self,
        folder: &Path,
        sources: &[Source],
        catalog: Option<(i64, String, String)>,
        show_missing: bool,
    ) {
        if self.shows.contains_key(folder) || self.pending.contains_key(folder) {
            return;
        }
        let (sender, receiver) = mpsc::channel();
        self.pending.insert(folder.into(), receiver);
        let folder = folder.to_path_buf();
        let sources = sources.to_vec();
        let cancel = self.cancel.clone();
        std::thread::spawn(move || {
            let mut loaded = load(&folder, &sources);
            if let Some((id, token, language)) = catalog {
                let check_missing =
                    show_missing && loaded.warnings.is_empty() && loaded.unrecognized == 0;
                if show_missing && !check_missing {
                    loaded.warnings.push("Missing-episode check skipped: some files could not be read or their episode numbers were not recognized.".into());
                }
                let seasons = loaded.incomplete_seasons();
                if check_missing || (!token.trim().is_empty() && !seasons.is_empty()) {
                    loaded.checking_metadata = true;
                    if sender.send(loaded.clone()).is_err() {
                        return;
                    }
                    if check_missing {
                        match media_shelf::metadata::episode_catalog_details(
                            id, &token, &language, &cancel,
                        ) {
                            Ok(catalog) => loaded.apply_catalog(catalog),
                            Err(e) => loaded.warnings.push(format!("Missing-episode check: {e}")),
                        }
                    } else {
                        for season in seasons {
                            if cancel.load(Ordering::Relaxed) {
                                return;
                            }
                            match media_shelf::metadata::season_episodes(
                                id, season, &token, &language,
                            ) {
                                Ok(details) => loaded.fill_details(season, &details),
                                Err(e) => loaded
                                    .warnings
                                    .push(format!("Episode details for season {season}: {e}")),
                            }
                        }
                    }
                    loaded.checking_metadata = false;
                }
            }
            let _ = sender.send(loaded);
        });
    }

    pub fn poll(&mut self, ctx: &egui::Context) {
        self.pending.retain(|folder, receiver| {
            loop {
                match receiver.try_recv() {
                    Ok(show) => {
                        self.shows.insert(folder.clone(), show);
                    }
                    Err(mpsc::TryRecvError::Empty) => return true,
                    Err(mpsc::TryRecvError::Disconnected) => {
                        self.shows.entry(folder.clone()).or_insert_with(|| Show {
                            warnings: vec!["Episode loading stopped. Refresh to try again.".into()],
                            ..Default::default()
                        });
                        return false;
                    }
                }
            }
        });
        if !self.pending.is_empty() {
            ctx.request_repaint_after(Duration::from_millis(100));
        }
    }

    pub fn episode(&self, path: &Path) -> Option<&Episode> {
        self.shows
            .values()
            .flat_map(|s| s.seasons.values().flatten())
            .find(|e| e.video.path == path)
    }
}

pub(super) fn load(folder: &Path, sources: &[Source]) -> Show {
    let mut show = Show::default();
    let mut files = vec![];
    for entry in walkdir::WalkDir::new(folder)
        .min_depth(1)
        .follow_links(false)
        .follow_root_links(false)
        .into_iter()
        .filter_entry(|e| {
            !media_shelf::identification::ignored_folder(&e.file_name().to_string_lossy())
        })
    {
        match entry {
            Ok(e) if e.file_type().is_file() => files.push(e.into_path()),
            Ok(_) => {}
            Err(e) => show.warnings.push(e.to_string()),
        }
    }
    if !folder.is_dir() {
        show.warnings
            .push(format!("Show folder unavailable: {}", folder.display()));
    }
    let source = sources
        .iter()
        .filter(|s| s.category == Category::Series && folder.starts_with(&s.path))
        .max_by_key(|s| s.path.components().count());
    for path in files
        .iter()
        .filter(|p| media_shelf::is_video(p) && !media_shelf::identification::sample_file(p))
    {
        if let Some((season, numbers)) = media_shelf::file_ops::episode_numbers(path) {
            show.present
                .extend(numbers.into_iter().map(|n| (season, n)));
        } else {
            show.unrecognized += 1;
        }
        let details = match media_shelf::file_ops::episode_details(path, &files) {
            Ok(details) => details,
            Err(e) => {
                show.warnings.push(format!("{}: {e}", path.display()));
                None
            }
        };
        let (season, number, label, needs_title, info) = match details {
            Some((s, n, info)) => {
                let needs_title = info.title.is_empty();
                let label = format!(
                    "{n}. {}",
                    if needs_title {
                        format!("Episode {n}")
                    } else {
                        info.title.clone()
                    }
                );
                (Some(s), Some(n), label, needs_title, info)
            }
            None => (
                None,
                None,
                path.file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into_owned(),
                false,
                EpisodeDetails::default(),
            ),
        };
        let bytes = match path.metadata() {
            Ok(m) => m.len(),
            Err(e) => {
                show.warnings.push(format!("{}: {e}", path.display()));
                continue;
            }
        };
        show.seasons.entry(season).or_default().push(Episode {
            video: Video {
                added_date: None,
                is_folder: false,
                category: Category::Series,
                path: path.clone(),
                relative_path: path
                    .strip_prefix(source.map_or(folder, |s| s.path.as_path()))
                    .unwrap_or(path)
                    .into(),
                bytes,
                tmdb: None,
                metadata: None,
                artwork: media_shelf::artwork::Artwork {
                    thumb: media_shelf::artwork::episode_thumb(path, &files),
                    ..Default::default()
                },
            },
            number,
            needs_title,
            label,
            details: info,
        });
    }
    for episodes in show.seasons.values_mut() {
        episodes.sort_by(|a, b| {
            a.number
                .cmp(&b.number)
                .then_with(|| a.video.path.cmp(&b.video.path))
        });
    }
    show
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn missing_local_title_is_filled_without_enabling_missing_episode_display() {
        let temp = tempfile::tempdir().unwrap();
        let folder = temp.path().join("The Rehearsal (2022)");
        let season = folder.join("Season 1");
        std::fs::create_dir_all(&season).unwrap();
        for number in [1, 6] {
            std::fs::write(
                season.join(format!("The Rehearsal - S01E{number:02}.mp4")),
                b"video",
            )
            .unwrap();
        }
        std::fs::write(season.join("The Rehearsal - S01E01 - Local Title.nfo"),"<episodedetails><title>Local Title</title><season>1</season><episode>1</episode><aired>2022-07-14</aired><ratings><rating default=\"true\" max=\"5\"><value>4.5</value></rating></ratings></episodedetails>").unwrap();
        let mut loaded = load(&folder, &[]);
        assert_eq!(loaded.incomplete_seasons(), vec![1]);
        assert_eq!(loaded.seasons[&Some(1)][1].label, "6. Episode 6");
        loaded.fill_details(
            1,
            &BTreeMap::from([
                (
                    1,
                    EpisodeDetails {
                        title: "Provider Title".into(),
                        aired: Some("2022-07-15".into()),
                        rating: Some(7.8),
                    },
                ),
                (
                    6,
                    EpisodeDetails {
                        title: "Pretend Daddy".into(),
                        aired: Some("2022-08-19".into()),
                        rating: Some(8.3),
                    },
                ),
                (
                    7,
                    EpisodeDetails {
                        title: "Not on disk".into(),
                        ..Default::default()
                    },
                ),
            ]),
        );
        assert_eq!(loaded.seasons[&Some(1)][0].label, "1. Local Title");
        assert_eq!(loaded.seasons[&Some(1)][1].label, "6. Pretend Daddy");
        assert!(loaded.incomplete_seasons().is_empty());
        assert_eq!(
            loaded.seasons[&Some(1)][0].details.aired.as_deref(),
            Some("2022-07-14")
        );
        assert_eq!(loaded.seasons[&Some(1)][0].details.rating, Some(9.0));
        assert_eq!(loaded.seasons[&Some(1)][1].details.rating, Some(8.3));
        assert!(
            loaded.missing.is_empty(),
            "Title lookup must not enable missing-episode rows"
        );
        assert_eq!(loaded.count(), 2);
        assert_eq!(
            std::fs::read_dir(&season).unwrap().count(),
            3,
            "Browsing titles must not rename files or create NFOs"
        );
    }

    #[test]
    fn missing_entries_include_whole_seasons_and_respect_multi_episode_files() {
        let temp = tempfile::tempdir().unwrap();
        let folder = temp.path().join("Show");
        std::fs::create_dir_all(folder.join("Season 1")).unwrap();
        let file = folder.join("Season 1/Show.S01E01E02.mkv");
        std::fs::write(&file, b"episodes 1 and 2").unwrap();
        let mut loaded = load(&folder, &[]);
        assert_eq!(loaded.unrecognized, 0);
        assert!(loaded.missing.is_empty());
        assert_eq!(
            media_shelf::file_ops::episode_numbers(Path::new("Show.S01E01-E03.mkv")),
            Some((1, vec![1, 2, 3]))
        );
        loaded.add_missing(BTreeMap::from([
            (
                1,
                BTreeMap::from([
                    (1, "Pilot".into()),
                    (2, "Second".into()),
                    (3, "Third".into()),
                ]),
            ),
            (2, BTreeMap::from([(1, "Return".into())])),
        ]));
        assert_eq!(loaded.missing[&1], BTreeMap::from([(3, "Third".into())]));
        assert_eq!(loaded.missing[&2], BTreeMap::from([(1, "Return".into())]));
        assert!(loaded.seasons[&Some(2)].is_empty());
        assert_eq!(
            loaded.count(),
            1,
            "Missing episodes must never become actionable video files"
        );
        assert!(file.exists());
        assert_eq!(
            std::fs::read_dir(&folder).unwrap().count(),
            1,
            "Missing seasons must not create folders"
        );
    }

    #[test]
    fn lazy_show_load_uses_numbered_nfos_and_numeric_order_without_changing_files() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();
        let folder = root.join("Show (2022)");
        for (season, number) in [(10, 1), (2, 10), (2, 2), (0, 1)] {
            let dir = folder.join(format!("Season {season}"));
            std::fs::create_dir_all(&dir).unwrap();
            std::fs::write(
                dir.join(format!("Show - S{season:02}E{number:02}.mp4")),
                b"video",
            )
            .unwrap();
        }
        std::fs::write(folder.join("Season 2/Show - S02E02 - Pilot.nfo"),
            "<episodedetails><title>Pilot</title><season>2</season><episode>2</episode></episodedetails>").unwrap();
        std::fs::write(folder.join("unrecognized.mkv"), b"unknown").unwrap();
        let sources = [Source {
            category: Category::Series,
            path: root.into(),
        }];
        let mut tree = SeriesTree::default();
        assert!(tree.shows.is_empty() && tree.pending.is_empty());
        tree.request(&folder, &sources, None, false);
        tree.request(&folder, &sources, None, false);
        assert_eq!(tree.pending.len(), 1);
        let show = tree
            .pending
            .remove(&folder)
            .unwrap()
            .recv_timeout(Duration::from_secs(5))
            .unwrap();
        assert!(show.warnings.is_empty(), "{:?}", show.warnings);
        assert_eq!(
            show.seasons.keys().copied().collect::<Vec<_>>(),
            vec![None, Some(0), Some(2), Some(10)]
        );
        assert_eq!(show.count(), 5);
        let episodes = &show.seasons[&Some(2)];
        assert_eq!(episodes[0].label, "2. Pilot");
        assert_eq!(episodes[1].label, "10. Episode 10");
        assert_eq!(episodes[0].video.bytes, 5);
        assert_eq!(
            episodes[0].video.relative_path,
            Path::new("Show (2022)/Season 2/Show - S02E02.mp4")
        );
        assert!(episodes[0].video.path.exists());
        tree.shows.insert(folder.clone(), show);
        tree.request(&folder, &sources, None, false);
        assert!(
            tree.pending.is_empty(),
            "Already loaded shows must not be rescanned"
        );
        assert_eq!(
            tree.episode(&folder.join("Season 2/Show - S02E02.mp4"))
                .unwrap()
                .label,
            "2. Pilot"
        );
        let missing = load(&root.join("disconnected"), &sources);
        assert!(!missing.warnings.is_empty());
    }
}
