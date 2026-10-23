//! Episode browsing is loaded on demand; the saved library stays a shallow show index.
use super::*;
use std::collections::{BTreeMap, HashMap};
use std::path::Path;

#[derive(Clone)]
pub(super) struct Episode {
    pub video: Video,
    pub number: Option<u32>,
    pub label: String,
}

#[derive(Clone, Default)]
pub(super) struct Show {
    pub seasons: BTreeMap<Option<u32>, Vec<Episode>>,
    pub warnings: Vec<String>,
    pub missing: BTreeMap<u32, BTreeMap<u32, String>>,
    pub checking_missing: bool,
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
            for (number, title) in episodes {
                if !self.present.contains(&(season, number)) {
                    self.missing
                        .entry(season)
                        .or_default()
                        .insert(number, title);
                    self.seasons.entry(Some(season)).or_default();
                } else if let Some(episode) = self
                    .seasons
                    .get_mut(&Some(season))
                    .and_then(|episodes| episodes.iter_mut().find(|e| e.number == Some(number)))
                {
                    if episode.label == format!("{number}. Episode {number}") {
                        episode.label = format!("{number}. {title}");
                    }
                }
            }
        }
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
                if !loaded.warnings.is_empty() || loaded.unrecognized > 0 {
                    loaded.warnings.push("Missing-episode check skipped: some files could not be read or their episode numbers were not recognized.".into());
                } else {
                    loaded.checking_missing = true;
                    if sender.send(loaded.clone()).is_err() {
                        return;
                    }
                    match media_shelf::metadata::episode_catalog(id, &token, &language, &cancel) {
                        Ok(catalog) => loaded.add_missing(catalog),
                        Err(e) => loaded.warnings.push(format!("Missing-episode check: {e}")),
                    }
                    loaded.checking_missing = false;
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
        let (season, number, label) = match details {
            Some((s, n, title)) => (
                Some(s),
                Some(n),
                format!("{n}. {}", title.unwrap_or_else(|| format!("Episode {n}"))),
            ),
            None => (
                None,
                None,
                path.file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into_owned(),
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
                artwork: Default::default(),
            },
            number,
            label,
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
        tree.request(&folder, &sources, None);
        tree.request(&folder, &sources, None);
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
        tree.request(&folder, &sources, None);
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
