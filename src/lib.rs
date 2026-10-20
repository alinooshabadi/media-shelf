//! Read-only media discovery, shared by the desktop and headless entry points.
pub mod artwork;
pub mod credentials;
pub mod file_ops;
pub mod identification;
pub mod metadata;
pub mod nfo;
pub mod storage;
pub mod tmdb;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashSet,
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
};
use walkdir::WalkDir;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Category {
    Movies,
    Series,
}

impl Category {
    pub fn label(self) -> &'static str {
        match self {
            Self::Movies => "Movies",
            Self::Series => "Series",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Source {
    pub category: Category,
    pub path: PathBuf,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Video {
    #[serde(default)]
    pub artwork: artwork::Artwork,
    pub category: Category,
    pub path: PathBuf,
    pub relative_path: PathBuf,
    pub bytes: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tmdb: Option<tmdb::TmdbMatch>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<metadata::Metadata>,
}

#[derive(Default, Debug, Serialize, Deserialize)]
pub struct ScanReport {
    pub videos: Vec<Video>,
    pub warnings: Vec<String>,
    pub cancelled: bool,
}

pub fn is_video(path: &Path) -> bool {
    path.extension().and_then(|e| e.to_str()).is_some_and(|e| {
        matches!(
            e.to_ascii_lowercase().as_str(),
            "mkv"
                | "mp4"
                | "m4v"
                | "avi"
                | "mov"
                | "wmv"
                | "webm"
                | "mpg"
                | "mpeg"
                | "ts"
                | "m2ts"
                | "mts"
                | "vob"
                | "ogv"
                | "flv"
                | "divx"
        )
    })
}

/// Recurses without following symlinks/junctions. Overlapping roots are deduplicated
/// within a category; the same file may intentionally belong to both categories.
pub fn scan(
    sources: &[Source],
    cancel: &AtomicBool,
    mut progress: impl FnMut(usize),
) -> ScanReport {
    let mut report = ScanReport::default();
    let mut seen = HashSet::new();
    for source in sources {
        if cancel.load(Ordering::Relaxed) {
            report.cancelled = true;
            break;
        }
        if !source.path.is_dir() {
            report
                .warnings
                .push(format!("Folder unavailable: {}", source.path.display()));
            continue;
        }
        let root = match source.path.canonicalize() {
            Ok(path) => path,
            Err(e) => {
                report
                    .warnings
                    .push(format!("{}: {e}", source.path.display()));
                continue;
            }
        };
        for entry in WalkDir::new(&root)
            .follow_links(false)
            .follow_root_links(false)
            .into_iter()
            .filter_entry(|e| !identification::ignored_folder(&e.file_name().to_string_lossy()))
        {
            if cancel.load(Ordering::Relaxed) {
                report.cancelled = true;
                break;
            }
            let entry = match entry {
                Ok(entry) => entry,
                Err(e) => {
                    report.warnings.push(e.to_string());
                    continue;
                }
            };
            if !entry.file_type().is_file()
                || !is_video(entry.path())
                || identification::sample_file(entry.path())
            {
                continue;
            }
            let key = (source.category.label(), entry.path().to_path_buf());
            if !seen.insert(key) {
                continue;
            }
            let metadata = match entry.metadata() {
                Ok(metadata) => metadata,
                Err(e) => {
                    report.warnings.push(e.to_string());
                    continue;
                }
            };
            report.videos.push(Video {
                artwork: Default::default(),
                category: source.category,
                path: entry.path().to_path_buf(),
                relative_path: entry
                    .path()
                    .strip_prefix(&root)
                    .unwrap_or(entry.path())
                    .to_path_buf(),
                bytes: metadata.len(),
                tmdb: None,
                metadata: None,
            });
            if report.videos.len() % 128 == 0 {
                progress(report.videos.len());
            }
        }
        if report.cancelled {
            break;
        }
    }
    report
        .videos
        .sort_by_cached_key(|v| (v.category.label(), v.path.to_string_lossy().to_lowercase()));
    progress(report.videos.len());
    artwork::attach(&mut report.videos, sources);
    report
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    static NEXT_FIXTURE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "media-shelf-test-{}-{}-{}",
                std::process::id(),
                NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn discovers_nested_movies_and_episodes_and_deduplicates_roots() {
        let f = Fixture::new();
        let movies = f.0.join("Movies");
        let season = f.0.join("Series/Show/Season 01");
        fs::create_dir_all(movies.join("Film (2024)")).unwrap();
        fs::create_dir_all(&season).unwrap();
        fs::write(movies.join("Film (2024)/Film.MKV"), b"video").unwrap();
        fs::write(movies.join("poster.jpg"), b"image").unwrap();
        fs::write(movies.join("unfinished.mp4.part"), b"partial").unwrap();
        fs::write(season.join("Show.S01E01.mp4"), b"episode").unwrap();
        let sources = vec![
            Source {
                category: Category::Movies,
                path: movies.clone(),
            },
            Source {
                category: Category::Movies,
                path: movies.join("Film (2024)"),
            },
            Source {
                category: Category::Series,
                path: f.0.join("Series"),
            },
        ];
        let report = scan(&sources, &AtomicBool::new(false), |_| {});
        assert_eq!(report.videos.len(), 2);
        assert_eq!(report.videos[0].bytes, 5);
        assert_eq!(report.videos[1].category, Category::Series);
        assert_eq!(
            report.videos[1].relative_path,
            PathBuf::from("Show/Season 01/Show.S01E01.mp4")
        );
        assert!(report.warnings.is_empty());
    }

    #[test]
    fn unavailable_root_does_not_hide_valid_results() {
        let f = Fixture::new();
        fs::write(f.0.join("movie.avi"), b"ok").unwrap();
        let sources = vec![
            Source {
                category: Category::Series,
                path: f.0.join("missing"),
            },
            Source {
                category: Category::Movies,
                path: f.0.clone(),
            },
        ];
        let report = scan(&sources, &AtomicBool::new(false), |_| {});
        assert_eq!(report.videos.len(), 1);
        assert_eq!(report.warnings.len(), 1);
        let cancelled = scan(&sources, &AtomicBool::new(true), |_| {});
        assert!(cancelled.cancelled);
        assert!(cancelled.videos.is_empty());
    }
}
