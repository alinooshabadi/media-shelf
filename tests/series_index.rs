use media_shelf::{
    Category, Source, index,
    metadata::{self, Field, Metadata, ScrapeOptions},
    scan,
    storage::LibraryDb,
    tmdb::TmdbMatch,
};
use std::{fs, sync::atomic::AtomicBool};

#[test]
fn series_index_only_lists_direct_show_folders_while_movies_stay_recursive() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().canonicalize().unwrap();
    for folder in [
        "Series/Show.One (2020)/Season 1/Deep",
        "Series/Empty Show",
        "Series/.deletedByTMM/Hidden",
        "Movies/Film/Deep",
    ] {
        fs::create_dir_all(root.join(folder)).unwrap();
    }
    for file in [
        "Series/Show.One (2020)/Season 1/Deep/show.S01E01.mkv",
        "Series/flat.S01E01.mp4",
        "Series/.deletedByTMM/Hidden/episode.mp4",
        "Movies/Film/Deep/film.mkv",
    ] {
        fs::write(root.join(file), b"fixture").unwrap();
    }
    #[cfg(unix)]
    std::os::unix::fs::symlink(
        root.join("Series/Show.One (2020)"),
        root.join("Series/Linked show"),
    )
    .unwrap();
    let sources = vec![
        Source {
            category: Category::Series,
            path: root.join("Series"),
        },
        Source {
            category: Category::Movies,
            path: root.join("Movies"),
        },
    ];
    let mut progress = vec![];
    let report = index(&sources, &AtomicBool::new(false), |n| progress.push(n));
    assert!(report.warnings.is_empty(), "{:?}", report.warnings);
    assert_eq!(report.videos.len(), 3);
    let shows: Vec<_> = report
        .videos
        .iter()
        .filter(|v| v.category == Category::Series)
        .collect();
    assert_eq!(shows.len(), 2);
    assert!(
        shows
            .iter()
            .all(|v| v.is_folder && v.bytes == 0 && v.relative_path.components().count() == 1)
    );
    assert!(
        shows
            .iter()
            .all(|v| metadata::series_folder(v, &sources).as_ref() == Some(&v.path))
    );
    assert!(shows.iter().all(|v| v.artwork.poster.is_none()));
    let mut selected = (**shows
        .iter()
        .find(|v| v.path.ends_with("Show.One (2020)"))
        .unwrap())
    .clone();
    fs::write(selected.path.join("poster.jpg"), b"poster").unwrap();
    media_shelf::artwork::attach_one(&mut selected, &sources);
    assert!(
        selected
            .artwork
            .poster
            .unwrap()
            .ends_with("Show.One (2020)/poster.jpg")
    );
    assert!(!report.videos[0].is_folder);
    assert!(report.videos[0].path.ends_with("Film/Deep/film.mkv"));
    assert_eq!(progress.last(), Some(&3));
    let cancelled = index(&sources, &AtomicBool::new(true), |_| {});
    assert!(cancelled.cancelled && cancelled.videos.is_empty());
}

#[test]
fn shallow_rescan_upgrades_old_episode_library_without_losing_show_metadata() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().canonicalize().unwrap();
    fs::create_dir_all(root.join("Show/Season 1")).unwrap();
    fs::write(root.join("Show/Season 1/Show.S01E01.mkv"), b"episode").unwrap();
    let sources = vec![Source {
        category: Category::Series,
        path: root.clone(),
    }];
    let path = root.join("library.sqlite3");
    let mut db = LibraryDb::open(&path).unwrap();
    db.set_sources(&sources).unwrap();
    let deep = scan(&sources, &AtomicBool::new(false), |_| {});
    db.apply_scan(&sources, &deep).unwrap();
    let identity = TmdbMatch {
        category: Category::Series,
        id: 42,
        title: "Matched Show".into(),
        original_title: "Matched Show".into(),
        release_date: Some("2020-01-01".into()),
        overview: "Show plot".into(),
    };
    let mut saved = Metadata::empty(identity.clone());
    saved.year = Some(2020);
    db.save_scraped(&[(Category::Series, deep.videos[0].path.clone(), saved.clone())])
        .unwrap();
    drop(db);
    let sql = rusqlite::Connection::open(&path).unwrap();
    sql.execute_batch("ALTER TABLE videos DROP COLUMN is_folder; PRAGMA user_version=4;")
        .unwrap();
    drop(sql);
    let mut db = LibraryDb::open(&path).unwrap();
    let shallow = index(&sources, &AtomicBool::new(false), |_| {});
    db.apply_scan(&sources, &shallow).unwrap();
    drop(db);
    let mut db = LibraryDb::open(&path).unwrap();
    let videos = db.videos().unwrap();
    assert_eq!(videos.len(), 1);
    assert!(videos[0].is_folder);
    assert_eq!(videos[0].tmdb, Some(identity));
    assert_eq!(videos[0].metadata, Some(saved.clone()));
    let sql = rusqlite::Connection::open(&path).unwrap();
    assert_eq!(
        sql.query_row("SELECT count(*) FROM scraped_metadata", [], |r| r
            .get::<_, i64>(0))
            .unwrap(),
        2
    );
    let options = ScrapeOptions {
        fields: vec![Field::Title, Field::Year],
        write_nfo: true,
        ..Default::default()
    };
    let (count, warnings) =
        metadata::commit_scrape(&mut db, &videos[0], &saved, &options, true).unwrap();
    assert_eq!(count, 1);
    assert!(warnings.is_empty(), "{warnings:?}");
    assert!(root.join("Show/tvshow.nfo").exists());
    let nfo = fs::read_to_string(root.join("Show/tvshow.nfo")).unwrap();
    assert!(nfo.contains("<tvshow>") && nfo.contains("Matched Show"));
    fs::rename(root.join("Show"), root.join("renamed-away")).unwrap();
    let next = index(&sources, &AtomicBool::new(false), |_| {});
    db.apply_scan(&sources, &next).unwrap();
    assert_eq!(db.videos().unwrap().len(), 1);
    assert!(db.videos().unwrap()[0].path.ends_with("renamed-away"));
    assert_eq!(
        sql.query_row("SELECT count(*) FROM scraped_metadata", [], |r| r
            .get::<_, i64>(0))
            .unwrap(),
        0
    );
}
