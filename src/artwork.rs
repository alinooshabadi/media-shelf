//! Discover existing artwork without changing files or creating provider matches.
use crate::{Category, Source, Video};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Artwork {
    pub poster: Option<PathBuf>,
    pub fanart: Option<PathBuf>,
    #[serde(default)]
    pub thumb: Option<PathBuf>,
}

/// Reuse the scanned directory entries; never traverse the NAS for a thumbnail.
pub fn episode_thumb(path: &Path, files: &[PathBuf]) -> Option<PathBuf> {
    let images: Vec<_> = files
        .iter()
        .filter(|p| {
            p.parent() == path.parent()
                && p.extension().is_some_and(|e| {
                    ["jpg", "jpeg", "png", "webp"]
                        .iter()
                        .any(|ext| e.eq_ignore_ascii_case(ext))
                })
        })
        .collect();
    let stem = path.file_stem()?.to_string_lossy();
    for name in [format!("{stem}-thumb"), format!("{stem}.thumb")] {
        if let Some(image) = images
            .iter()
            .find(|p| p.file_stem().is_some_and(|s| s.eq_ignore_ascii_case(&name)))
        {
            return Some((*image).clone());
        }
    }
    if let Some(numbers) = crate::file_ops::episode_numbers(path) {
        let matching: Vec<_> = images
            .iter()
            .filter(|p| {
                p.file_stem()
                    .is_some_and(|s| s.to_string_lossy().to_lowercase().ends_with("-thumb"))
                    && crate::file_ops::episode_numbers(p).as_ref() == Some(&numbers)
            })
            .collect();
        if matching.len() == 1 {
            return Some((**matching[0]).clone());
        }
    }
    images
        .iter()
        .find(|p| {
            p.file_stem()
                .is_some_and(|s| s.eq_ignore_ascii_case("thumb"))
        })
        .map(|p| (*p).clone())
}

pub fn attach(videos: &mut [Video], sources: &[Source]) {
    attach_items(videos.iter_mut().filter(|v| !v.is_folder), sources);
}

pub fn attach_one(video: &mut Video, sources: &[Source]) {
    attach_items(std::iter::once(video), sources);
}

pub(crate) fn attach_items<'a>(videos: impl Iterator<Item = &'a mut Video>, sources: &[Source]) {
    let mut folders: HashMap<PathBuf, HashMap<String, PathBuf>> = HashMap::new();
    for video in videos {
        let mut locations = vec![];
        if video.is_folder {
            locations.push(video.path.clone());
        } else if let Some(parent) = video.path.parent() {
            locations.push(parent.to_path_buf());
        }
        if video.category == Category::Series {
            if let Some(show) = crate::metadata::series_folder(video, sources) {
                if !locations.contains(&show) {
                    locations.push(show);
                }
            }
        }
        let stem = video
            .path
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .to_lowercase();
        let mut artwork = Artwork::default();
        for folder in locations {
            let files = folders.entry(folder.clone()).or_insert_with(|| {
                std::fs::read_dir(&folder)
                    .into_iter()
                    .flatten()
                    .filter_map(Result::ok)
                    .filter(|e| e.file_type().is_ok_and(|t| t.is_file()))
                    .map(|e| (e.file_name().to_string_lossy().to_lowercase(), e.path()))
                    .collect()
            });
            if video.category == Category::Series && !video.is_folder && artwork.thumb.is_none() {
                artwork.thumb =
                    episode_thumb(&video.path, &files.values().cloned().collect::<Vec<_>>());
            }
            for (target, names) in [
                (
                    &mut artwork.poster,
                    vec![
                        format!("{stem}-poster"),
                        format!("{stem}.poster"),
                        "poster".into(),
                        "folder".into(),
                    ],
                ),
                (
                    &mut artwork.fanart,
                    vec![
                        format!("{stem}-fanart"),
                        format!("{stem}.fanart"),
                        "fanart".into(),
                        "backdrop".into(),
                    ],
                ),
            ] {
                if target.is_some() {
                    continue;
                }
                *target = names.iter().find_map(|name| {
                    ["jpg", "jpeg", "png", "webp"]
                        .iter()
                        .find_map(|ext| files.get(&format!("{name}.{ext}")).cloned())
                });
            }
        }
        video.artwork = artwork;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn episode_thumbs_match_numbered_sidecars_and_prefer_exact_names() {
        let video = PathBuf::from("Show/Season 1/Show - S01E01.mp4");
        let numbered = PathBuf::from("Show/Season 1/Show - S01E01 - Pilot-thumb.JPG");
        let generic = PathBuf::from("Show/Season 1/thumb.jpg");
        let exact = PathBuf::from("Show/Season 1/Show - S01E01-thumb.png");
        let mut files = vec![
            generic.clone(),
            PathBuf::from("Other/Show - S01E01-thumb.jpg"),
            PathBuf::from("Show/Season 1/Show - S01E02 - Wrong-thumb.jpg"),
            PathBuf::from("Show/Season 1/Show - S01E01 - Wrong-poster.jpg"),
            numbered.clone(),
        ];
        assert_eq!(episode_thumb(&video, &files), Some(numbered.clone()));
        files.push(exact.clone());
        assert_eq!(episode_thumb(&video, &files), Some(exact.clone()));
        files.retain(|p| p != &exact && p != &numbered);
        assert_eq!(episode_thumb(&video, &files), Some(generic.clone()));
        files.retain(|p| p != &generic);
        assert_eq!(episode_thumb(&video, &files), None);
        let old: Artwork = serde_json::from_str(r#"{"poster":null,"fanart":null}"#).unwrap();
        assert!(old.thumb.is_none());
    }

    #[test]
    fn filename_specific_artwork_wins_and_tv_show_art_is_discovered() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();
        std::fs::create_dir_all(root.join("Show/Season 01")).unwrap();
        for file in [
            "film.mkv",
            "poster.jpg",
            "film-poster.png",
            "Show/poster.jpeg",
            "Show/fanart.webp",
            "Show/Season 01/episode.mkv",
        ] {
            std::fs::write(root.join(file), b"fixture").unwrap();
        }
        let sources = vec![Source {
            category: Category::Movies,
            path: root.clone(),
        }];
        let report = crate::scan(&sources, &std::sync::atomic::AtomicBool::new(false), |_| {});
        let film = report
            .videos
            .iter()
            .find(|v| v.path.ends_with("film.mkv"))
            .unwrap();
        assert!(
            film.artwork
                .poster
                .as_ref()
                .unwrap()
                .ends_with("film-poster.png")
        );
        let report = crate::scan(
            &[Source {
                category: Category::Series,
                path: root,
            }],
            &std::sync::atomic::AtomicBool::new(false),
            |_| {},
        );
        let episode = report
            .videos
            .iter()
            .find(|v| v.path.ends_with("episode.mkv"))
            .unwrap();
        assert!(
            episode
                .artwork
                .poster
                .as_ref()
                .unwrap()
                .ends_with("Show/poster.jpeg")
        );
        assert!(
            episode
                .artwork
                .fanart
                .as_ref()
                .unwrap()
                .ends_with("Show/fanart.webp")
        );
    }
    #[test]
    fn existing_artwork_loads_without_scraping_and_refreshes_after_reopen() {
        let dir = tempfile::tempdir().unwrap();
        let folder = dir.path().join("1960 - Psycho");
        std::fs::create_dir(&folder).unwrap();
        std::fs::write(folder.join("movie.avi"), b"video").unwrap();
        for name in ["POSTER.JPG", "fanart.png", "banner.jpg"] {
            std::fs::write(folder.join(name), b"artwork fixture").unwrap();
        }
        let sources = vec![Source {
            category: Category::Movies,
            path: dir.path().canonicalize().unwrap(),
        }];
        let report = crate::scan(&sources, &std::sync::atomic::AtomicBool::new(false), |_| {});
        assert!(
            report.videos[0]
                .artwork
                .poster
                .as_ref()
                .unwrap()
                .ends_with("POSTER.JPG")
        );
        assert!(
            report.videos[0]
                .artwork
                .fanart
                .as_ref()
                .unwrap()
                .ends_with("fanart.png")
        );
        assert!(report.videos[0].metadata.is_none());
        let mut db = crate::storage::LibraryDb::open(&dir.path().join("library.sqlite3")).unwrap();
        db.set_sources(&sources).unwrap();
        db.apply_scan(&sources, &report).unwrap();
        drop(db);
        let db = crate::storage::LibraryDb::open(&dir.path().join("library.sqlite3")).unwrap();
        assert!(db.videos().unwrap()[0].artwork.poster.is_some());
        std::fs::remove_file(folder.join("POSTER.JPG")).unwrap();
        assert!(db.videos().unwrap()[0].artwork.poster.is_none());
    }
}
