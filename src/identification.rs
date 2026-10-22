//! Local title hints, independent of a confirmed provider match.
use crate::Video;
use regex::Regex;
use std::sync::LazyLock;

static FOLDER: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(19\d{2}|20\d{2})\s+-\s+(.+)$").unwrap());
static PAREN_YEAR: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(.+?)\s*\((19\d{2}|20\d{2})\)").unwrap());
static RELEASE_YEAR: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(.+?)[ ._-]+(19\d{2}|20\d{2})(?:\b|[._-])").unwrap());
static TECH: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)[ ._-]+(?:h\.?26[45]|x26[45]|xvid|\d{3,4}p|bluray|blu-ray|web-dl|webrip|aac|ac3|dts|mp3)(?:\b|[._-]).*$").unwrap()
});
static SAMPLE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)(?:^|[ ._-])sample$").unwrap());

pub fn ignored_folder(name: &str) -> bool {
    [".mediashelf-cleanup", ".deletedbytmm", "sample", "samples"]
        .iter()
        .any(|n| name.eq_ignore_ascii_case(n))
}
pub fn sample_file(path: &std::path::Path) -> bool {
    path.file_stem()
        .is_some_and(|s| SAMPLE.is_match(&s.to_string_lossy()))
}
pub fn local_identity(video: &Video) -> (String, Option<u16>) {
    // Only examine folders within this video's source-relative path.
    let levels = video.relative_path.components().count().saturating_sub(1);
    for folder in video.path.ancestors().skip(1).take(levels) {
        if let Some(name) = folder.file_name() {
            if let Some(c) = FOLDER.captures(&name.to_string_lossy()) {
                return (c[2].trim().to_string(), c[1].parse().ok());
            }
        }
    }
    let raw = video.path.file_stem().unwrap_or_default().to_string_lossy();
    for re in [&*PAREN_YEAR, &*RELEASE_YEAR] {
        if let Some(c) = re.captures(&raw) {
            return (
                c[1].replace(['.', '_'], " ").trim().to_string(),
                c[2].parse().ok(),
            );
        }
    }
    (
        TECH.replace(&raw, "")
            .replace(['.', '_'], " ")
            .trim()
            .to_string(),
        None,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Category;
    #[test]
    fn scanner_excludes_sample_clips_and_tmm_deleted_content() {
        let dir = tempfile::tempdir().unwrap();
        for folder in ["1976 - Taxi Driver", ".deletedByTMM/Old Movie", "samples"] {
            std::fs::create_dir_all(dir.path().join(folder)).unwrap();
        }
        for path in [
            "1976 - Taxi Driver/movie.mp4",
            "1976 - Taxi Driver/movie-sample.mp4",
            ".deletedByTMM/Old Movie/movie.mp4",
            "samples/clip.mp4",
        ] {
            std::fs::write(dir.path().join(path), b"video fixture").unwrap();
        }
        let report = crate::scan(
            &[crate::Source {
                category: Category::Movies,
                path: dir.path().to_path_buf(),
            }],
            &std::sync::atomic::AtomicBool::new(false),
            |_| {},
        );
        assert_eq!(report.videos.len(), 1);
        assert_eq!(
            local_identity(&report.videos[0]),
            ("Taxi Driver".into(), Some(1976))
        );
    }
    #[test]
    fn folder_title_and_year_override_release_filename() {
        for (path, expected, year) in [
            (
                "1976 - Taxi Driver/release.h264.1080p.mp4",
                "Taxi Driver",
                Some(1976),
            ),
            ("1984 - 1984/video.mp4", "1984", Some(1984)),
            (
                "10 Cloverfield Lane (2016) h264-720p AAC-2ch.mp4",
                "10 Cloverfield Lane",
                Some(2016),
            ),
            (
                "Blade Runner 2049 (2017).mp4",
                "Blade Runner 2049",
                Some(2017),
            ),
            (
                "2001.A.Space.Odyssey.1968.1080p.mkv",
                "2001 A Space Odyssey",
                Some(1968),
            ),
        ] {
            let video = Video {
                is_folder: false,
                artwork: Default::default(),
                category: Category::Movies,
                path: std::path::Path::new("I:/Movies/Movies").join(path),
                relative_path: path.into(),
                bytes: 0,
                tmdb: None,
                metadata: None,
            };
            assert_eq!(local_identity(&video), (expected.into(), year));
        }
        assert!(sample_file(std::path::Path::new(
            "127 Hours (2010)-sample.mkv"
        )));
        assert!(!sample_file(std::path::Path::new(
            "A Sample of Life (2020).mkv"
        )));
    }
}
