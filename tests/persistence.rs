use media_shelf::{Category, ScanReport, Source, Video, scan, storage::LibraryDb};
use std::{fs, path::PathBuf, sync::atomic::AtomicBool};
static NEXT_FIXTURE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

struct Fixture {
    root: PathBuf,
}
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "media-shelf-db-{}-{}-{}",
            std::process::id(),
            NEXT_FIXTURE.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(root.join("Movies")).unwrap();
        fs::create_dir_all(root.join("Series/Show/Season 01")).unwrap();
        Self {
            root: root.canonicalize().unwrap(),
        }
    }
    fn db(&self) -> PathBuf {
        self.root.join("library.sqlite3")
    }
    fn sources(&self) -> Vec<Source> {
        vec![
            Source {
                category: Category::Movies,
                path: self.root.join("Movies"),
            },
            Source {
                category: Category::Series,
                path: self.root.join("Series"),
            },
        ]
    }
    fn scan(&self) -> ScanReport {
        scan(&self.sources(), &AtomicBool::new(false), |_| {})
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

#[test]
fn chosen_matches_survive_reopen_rescan_and_can_be_changed_or_cleared() {
    use media_shelf::tmdb::TmdbMatch;
    let f = Fixture::new();
    let movie = f.root.join("Movies/film.mp4");
    fs::write(&movie, b"film").unwrap();
    let matched = TmdbMatch {
        category: Category::Movies,
        id: 42,
        title: "A Film".into(),
        original_title: "Original Film".into(),
        release_date: Some("2024-01-02".into()),
        overview: "An overview".into(),
    };
    {
        let mut db = LibraryDb::open(&f.db()).unwrap();
        db.set_sources(&f.sources()).unwrap();
        db.apply_scan(&f.sources(), &f.scan()).unwrap();
        db.save_match(Category::Movies, &movie, &matched).unwrap();
    }
    let mut db = LibraryDb::open(&f.db()).unwrap();
    assert_eq!(db.videos().unwrap()[0].tmdb.as_ref(), Some(&matched));
    fs::write(&movie, b"longer file").unwrap();
    db.apply_scan(&f.sources(), &f.scan()).unwrap();
    assert_eq!(db.videos().unwrap()[0].tmdb.as_ref(), Some(&matched));
    let mut replacement = matched.clone();
    replacement.id = 99;
    replacement.title = "Corrected Film".into();
    db.save_match(Category::Movies, &movie, &replacement)
        .unwrap();
    assert_eq!(db.videos().unwrap()[0].tmdb.as_ref(), Some(&replacement));
    assert!(db.save_match(Category::Series, &movie, &matched).is_err());
    assert!(
        db.save_match(
            Category::Movies,
            &f.root.join("Movies/missing.mp4"),
            &matched
        )
        .is_err()
    );
    db.clear_match(Category::Movies, &movie).unwrap();
    assert!(db.videos().unwrap()[0].tmdb.is_none());
    db.save_match(Category::Movies, &movie, &matched).unwrap();
    fs::remove_file(movie).unwrap();
    db.apply_scan(&f.sources(), &f.scan()).unwrap();
    let sql = rusqlite::Connection::open(f.db()).unwrap();
    assert_eq!(
        sql.query_row("SELECT count(*) FROM tmdb_matches", [], |r| r
            .get::<_, i64>(0))
            .unwrap(),
        0
    );
}

#[test]
fn upgrading_a_v02_database_preserves_library_and_supports_series_matches() {
    use media_shelf::tmdb::TmdbMatch;
    let f = Fixture::new();
    let episode = f.root.join("Series/Show/episode.mkv");
    fs::write(&episode, b"episode").unwrap();
    {
        let mut db = LibraryDb::open(&f.db()).unwrap();
        db.set_sources(&f.sources()).unwrap();
        db.apply_scan(&f.sources(), &f.scan()).unwrap();
    }
    {
        let sql = rusqlite::Connection::open(f.db()).unwrap();
        sql.execute_batch("DROP TABLE tmdb_matches; PRAGMA user_version=1;")
            .unwrap();
    }
    let mut db = LibraryDb::open(&f.db()).unwrap();
    assert_eq!(db.sources().unwrap(), f.sources());
    assert_eq!(db.videos().unwrap().len(), 1);
    let show = TmdbMatch {
        category: Category::Series,
        id: 77,
        title: "Show".into(),
        original_title: "Show".into(),
        release_date: None,
        overview: String::new(),
    };
    db.save_match(Category::Series, &episode, &show).unwrap();
    assert_eq!(db.videos().unwrap()[0].tmdb.as_ref(), Some(&show));
    db.set_sources(&[]).unwrap();
    assert!(episode.exists());
    let sql = rusqlite::Connection::open(f.db()).unwrap();
    assert_eq!(
        sql.query_row("SELECT count(*) FROM tmdb_matches", [], |r| r
            .get::<_, i64>(0))
            .unwrap(),
        0
    );
    assert_eq!(
        sql.pragma_query_value(None, "user_version", |r| r.get::<_, i64>(0))
            .unwrap(),
        5
    );
}

#[test]
fn files_survive_reopen_and_clean_rescan_reconciles_changes() {
    let f = Fixture::new();
    fs::write(f.root.join("Movies/Café (2024).MKV"), b"movie").unwrap();
    let episode = f.root.join("Series/Show/Season 01/Show.S01E01.mp4");
    fs::write(&episode, b"episode").unwrap();
    {
        let mut db = LibraryDb::open(&f.db()).unwrap();
        db.set_sources(&f.sources()).unwrap();
        db.apply_scan(&f.sources(), &f.scan()).unwrap();
    }
    assert_eq!(&fs::read(f.db()).unwrap()[..16], b"SQLite format 3\0");
    let mut db = LibraryDb::open(&f.db()).unwrap();
    assert_eq!(db.sources().unwrap(), f.sources());
    let videos = db.videos().unwrap();
    assert_eq!(videos.len(), 2);
    assert_eq!(videos[0].bytes, 5);
    assert_eq!(
        videos[1].relative_path,
        PathBuf::from("Show/Season 01/Show.S01E01.mp4")
    );
    fs::remove_file(episode).unwrap();
    fs::write(f.root.join("Movies/Café (2024).MKV"), b"updated movie").unwrap();
    db.apply_scan(&f.sources(), &f.scan()).unwrap();
    assert_eq!(db.videos().unwrap().len(), 1);
    assert_eq!(db.videos().unwrap()[0].bytes, 13);
    // Repeating the scan updates the same record instead of adding duplicates.
    db.apply_scan(&f.sources(), &f.scan()).unwrap();
    assert_eq!(db.videos().unwrap().len(), 1);
    let sql = rusqlite::Connection::open(f.db()).unwrap();
    let timestamps: (i64, i64) = sql
        .query_row("SELECT first_seen_at, last_seen_at FROM videos", [], |r| {
            Ok((r.get(0)?, r.get(1)?))
        })
        .unwrap();
    assert!(timestamps.0 > 0 && timestamps.1 >= timestamps.0);
}

#[test]
fn offline_and_cancelled_scans_keep_the_saved_library() {
    let f = Fixture::new();
    fs::write(f.root.join("Movies/movie.mp4"), b"movie").unwrap();
    fs::write(f.root.join("Series/Show/episode.mkv"), b"episode").unwrap();
    let mut db = LibraryDb::open(&f.db()).unwrap();
    db.set_sources(&f.sources()).unwrap();
    db.apply_scan(&f.sources(), &f.scan()).unwrap();
    fs::rename(f.root.join("Series"), f.root.join("offline-series")).unwrap();
    fs::write(f.root.join("Movies/new.mkv"), b"new").unwrap();
    let report = f.scan();
    assert!(!report.warnings.is_empty());
    db.apply_scan(&f.sources(), &report).unwrap();
    assert_eq!(db.videos().unwrap().len(), 3);
    let cancelled = ScanReport {
        videos: vec![],
        warnings: vec![],
        cancelled: true,
    };
    db.apply_scan(&f.sources(), &cancelled).unwrap();
    assert_eq!(db.videos().unwrap().len(), 3);
}

#[test]
fn removing_a_source_prunes_only_its_cache_and_keeps_files_and_overlaps() {
    let f = Fixture::new();
    let film = f.root.join("Movies/film.mp4");
    let episode = f.root.join("Series/Show/episode.mkv");
    fs::write(&film, b"movie").unwrap();
    fs::write(&episode, b"episode").unwrap();
    let mut sources = f.sources();
    sources.push(Source {
        category: Category::Series,
        path: f.root.join("Series/Show"),
    });
    let mut db = LibraryDb::open(&f.db()).unwrap();
    db.set_sources(&sources).unwrap();
    db.apply_scan(&sources, &scan(&sources, &AtomicBool::new(false), |_| {}))
        .unwrap();
    db.set_sources(&sources[2..]).unwrap();
    assert_eq!(db.videos().unwrap().len(), 1);
    assert_eq!(db.videos().unwrap()[0].category, Category::Series);
    assert!(film.exists() && episode.exists());
    db.set_sources(&[]).unwrap();
    assert!(db.videos().unwrap().is_empty());
    assert!(episode.exists());
}

#[test]
fn legacy_json_import_happens_once_and_keeps_the_backup() {
    let f = Fixture::new();
    let legacy = f.root.join("folders.json");
    fs::write(&legacy, serde_json::to_vec(&f.sources()).unwrap()).unwrap();
    {
        let mut db = LibraryDb::open(&f.db()).unwrap();
        db.migrate_folders(&legacy).unwrap();
        assert_eq!(db.sources().unwrap(), f.sources());
        db.set_sources(&[]).unwrap();
    }
    let mut db = LibraryDb::open(&f.db()).unwrap();
    db.migrate_folders(&legacy).unwrap();
    assert!(db.sources().unwrap().is_empty());
    assert!(legacy.exists());
}

#[test]
fn corrupt_legacy_json_can_be_fixed_and_retried() {
    let f = Fixture::new();
    let legacy = f.root.join("folders.json");
    fs::write(&legacy, b"broken JSON").unwrap();
    let mut db = LibraryDb::open(&f.db()).unwrap();
    assert!(db.migrate_folders(&legacy).is_err());
    fs::write(&legacy, serde_json::to_vec(&f.sources()).unwrap()).unwrap();
    db.migrate_folders(&legacy).unwrap();
    assert_eq!(db.sources().unwrap(), f.sources());
}

#[test]
fn failed_updates_roll_back_and_stale_scans_are_rejected() {
    let f = Fixture::new();
    fs::write(f.root.join("Movies/original.mp4"), b"original").unwrap();
    let mut db = LibraryDb::open(&f.db()).unwrap();
    db.set_sources(&f.sources()).unwrap();
    db.apply_scan(&f.sources(), &f.scan()).unwrap();
    let mut report = ScanReport::default();
    for (name, bytes) in [("good.mp4", 10), ("overflow.mp4", u64::MAX)] {
        report.videos.push(Video {
            is_folder: false,
            artwork: Default::default(),
            category: Category::Movies,
            path: f.root.join("Movies").join(name),
            relative_path: name.into(),
            bytes,
            tmdb: None,
            metadata: None,
        });
    }
    assert!(db.apply_scan(&f.sources(), &report).is_err());
    let saved = db.videos().unwrap();
    assert_eq!(saved.len(), 1);
    assert_eq!(saved[0].bytes, 8);
    let mut duplicate_sources = f.sources();
    duplicate_sources.push(duplicate_sources[0].clone());
    assert!(db.set_sources(&duplicate_sources).is_err());
    assert_eq!(db.sources().unwrap(), f.sources());
    db.set_sources(&f.sources()[..1]).unwrap();
    assert!(db.apply_scan(&f.sources(), &f.scan()).is_err());
    assert_eq!(db.videos().unwrap().len(), 1);
}
