use media_shelf::{Category, Source, Video, file_ops, scan, storage::LibraryDb, tmdb::TmdbMatch};
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicBool, AtomicU64, Ordering},
};
static NEXT: AtomicU64 = AtomicU64::new(0);
struct Fixture {
    root: PathBuf,
}
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "media-shelf-ops-{}-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed),
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
    fn dbpath(&self) -> PathBuf {
        self.root.join("library.sqlite3")
    }
    fn scan(&self) -> media_shelf::ScanReport {
        scan(&self.sources(), &AtomicBool::new(false), |_| {})
    }
    fn db(&self) -> LibraryDb {
        let mut db = LibraryDb::open(&self.dbpath()).unwrap();
        db.set_sources(&self.sources()).unwrap();
        db.apply_scan(&self.sources(), &self.scan()).unwrap();
        db
    }
    fn matched(&self, db: &mut LibraryDb, category: Category, path: &std::path::Path) -> Video {
        db.save_match(
            category,
            path,
            &TmdbMatch {
                category,
                id: 42,
                title: "A: Title?".into(),
                original_title: "Original".into(),
                release_date: Some("2024-01-01".into()),
                overview: "Story".into(),
            },
        )
        .unwrap();
        db.videos()
            .unwrap()
            .into_iter()
            .find(|v| v.category == category && v.path == path)
            .unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

#[test]
fn movie_organization_creates_year_title_folder_and_undo_restores_paths() {
    for existing_folder in [false, true] {
        let f = Fixture::new();
        let root = f.root.join("Movies");
        let folder = if existing_folder {
            root.join("old-release")
        } else {
            root.clone()
        };
        fs::create_dir_all(&folder).unwrap();
        let original = folder.join("release.mp4");
        fs::write(&original, b"movie fixture").unwrap();
        fs::write(folder.join("release.en.srt"), b"subtitle fixture").unwrap();
        fs::write(folder.join("release-poster.jpg"), b"art fixture").unwrap();
        fs::write(folder.join("release-fanart.jpg"), b"fanart fixture").unwrap();
        fs::write(folder.join("release.nfo"), b"nfo fixture").unwrap();
        let mut db = f.db();
        let mut video = f.matched(&mut db, Category::Movies, &original);
        let matched = video.tmdb.as_mut().unwrap();
        matched.title = "Fat Girl".into();
        matched.release_date = Some("2001-03-07".into());
        db.save_match(Category::Movies, &original, video.tmdb.as_ref().unwrap())
            .unwrap();
        let name = file_ops::suggested_name(&video);
        assert_eq!(name, "Fat Girl (2001).mp4");
        let plan = file_ops::organize_preview(&f.sources(), &[(video, name)]).unwrap();
        let destination = root.join("2001 - Fat Girl/Fat Girl (2001).mp4");
        assert_eq!(plan.moves[0].to, destination);
        assert!(
            !destination.parent().unwrap().exists(),
            "Preview must not create folders"
        );
        let id = file_ops::apply(&mut db, &plan).unwrap();
        assert!(destination.exists());
        assert!(destination.with_extension("en.srt").exists());
        assert!(destination.with_file_name("poster.jpg").exists());
        assert_eq!(db.videos().unwrap()[0].path, destination);
        assert!(destination.with_file_name("fanart.jpg").exists());
        assert!(destination.with_file_name("movie.nfo").exists());
        let current = db.videos().unwrap().remove(0);
        assert!(
            file_ops::organize_preview(
                &f.sources(),
                &[(current.clone(), file_ops::suggested_name(&current))]
            )
            .unwrap()
            .moves
            .is_empty()
        );
        drop(db);
        let mut db = LibraryDb::open(&f.dbpath()).unwrap();
        file_ops::undo(&mut db, id, &plan).unwrap();
        assert!(original.exists());
        assert!(folder.join("release.en.srt").exists());
        assert!(folder.join("release.nfo").exists());
        assert_eq!(db.videos().unwrap()[0].path, original);
    }
}

#[test]
fn generic_sidecars_win_when_multiple_sources_target_a_new_folder() {
    let f = Fixture::new();
    let folder = f.root.join("Movies/old-release");
    fs::create_dir(&folder).unwrap();
    let original = folder.join("release.mkv");
    fs::write(&original, b"video fixture").unwrap();
    for (generic, named) in [
        ("fanart.jpg", "release-fanart.jpg"),
        ("poster.jpg", "release-poster.jpg"),
        ("movie.nfo", "release.nfo"),
    ] {
        fs::write(folder.join(generic), b"preferred generic").unwrap();
        fs::write(folder.join(named), b"alternative named").unwrap();
    }
    let mut db = f.db();
    let mut video = f.matched(&mut db, Category::Movies, &original);
    let matched = video.tmdb.as_mut().unwrap();
    matched.title = "[REC]²".into();
    matched.release_date = Some("2009-01-01".into());
    let plan = file_ops::organize_preview(
        &f.sources(),
        &[(video.clone(), file_ops::suggested_name(&video))],
    )
    .unwrap();
    assert_eq!(plan.moves.len(), 4);
    let target = f.root.join("Movies/2009 - [REC]²");
    let id = file_ops::apply(&mut db, &plan).unwrap();
    assert!(target.join("[REC]² (2009).mkv").exists());
    for (generic, named) in [
        ("fanart.jpg", "release-fanart.jpg"),
        ("poster.jpg", "release-poster.jpg"),
        ("movie.nfo", "release.nfo"),
    ] {
        assert_eq!(
            fs::read(target.join(generic)).unwrap(),
            b"preferred generic"
        );
        assert_eq!(fs::read(folder.join(named)).unwrap(), b"alternative named");
    }
    file_ops::undo(&mut db, id, &plan).unwrap();
    assert!(original.exists());
    assert_eq!(
        fs::read(folder.join("fanart.jpg")).unwrap(),
        b"preferred generic"
    );
}

#[test]
fn existing_standard_sidecars_do_not_block_movie_rename() {
    let f = Fixture::new();
    let folder = f.root.join("Movies/1956 - Night and Fog");
    fs::create_dir(&folder).unwrap();
    let stem = "Night and Fog (1956) DivX-480p AC3-2ch";
    let original = folder.join(format!("{stem}.avi"));
    fs::write(&original, b"video fixture").unwrap();
    fs::write(folder.join(format!("{stem}.srt")), b"subtitle fixture").unwrap();
    for (standard, old) in [
        ("fanart.jpg", format!("{stem}-fanart.jpg")),
        ("poster.jpg", format!("{stem}-poster.jpg")),
        ("movie.nfo", format!("{stem}.nfo")),
    ] {
        fs::write(folder.join(standard), b"keep existing standard").unwrap();
        fs::write(folder.join(old), b"keep alternative too").unwrap();
    }
    let mut db = f.db();
    let mut video = f.matched(&mut db, Category::Movies, &original);
    let m = video.tmdb.as_mut().unwrap();
    m.title = "Night and Fog".into();
    m.release_date = Some("1956-01-01".into());
    let plan = file_ops::organize_preview(
        &f.sources(),
        &[(video.clone(), file_ops::suggested_name(&video))],
    )
    .unwrap();
    assert_eq!(
        plan.moves.len(),
        2,
        "Only the video and subtitle need moving"
    );
    assert!(
        plan.notes
            .iter()
            .any(|n| n.contains("Kept existing sidecar"))
    );
    let id = file_ops::apply(&mut db, &plan).unwrap();
    assert!(folder.join("Night and Fog (1956).avi").exists());
    for (standard, old) in [
        ("fanart.jpg", format!("{stem}-fanart.jpg")),
        ("poster.jpg", format!("{stem}-poster.jpg")),
        ("movie.nfo", format!("{stem}.nfo")),
    ] {
        assert_eq!(
            fs::read(folder.join(standard)).unwrap(),
            b"keep existing standard"
        );
        assert_eq!(fs::read(folder.join(old)).unwrap(), b"keep alternative too");
    }
    file_ops::undo(&mut db, id, &plan).unwrap();
    assert!(original.exists());
    assert!(folder.join(format!("{stem}.srt")).exists());
}

#[test]
fn correctly_named_movie_still_standardizes_sidecars() {
    let f = Fixture::new();
    let folder = f.root.join("Movies/2024 - A_ Title_");
    fs::create_dir(&folder).unwrap();
    let original = folder.join("A_ Title_ (2024).mp4");
    fs::write(&original, b"video fixture").unwrap();
    fs::write(original.with_extension("nfo"), b"nfo fixture").unwrap();
    fs::write(
        folder.join("A_ Title_ (2024)-poster.jpg"),
        b"poster fixture",
    )
    .unwrap();
    let mut db = f.db();
    let video = f.matched(&mut db, Category::Movies, &original);
    let plan = file_ops::organize_preview(
        &f.sources(),
        &[(video.clone(), file_ops::suggested_name(&video))],
    )
    .unwrap();
    assert_eq!(plan.moves.len(), 2);
    assert!(plan.moves.iter().all(|m| !m.video));
    let id = file_ops::apply(&mut db, &plan).unwrap();
    assert!(folder.join("movie.nfo").exists());
    assert!(folder.join("poster.jpg").exists());
    file_ops::undo(&mut db, id, &plan).unwrap();
    assert!(original.with_extension("nfo").exists());
    fs::remove_file(folder.join("A_ Title_ (2024)-poster.jpg")).unwrap();
    fs::write(folder.join("poster.jpg"), b"generic poster").unwrap();
    let mut changed = video;
    changed.tmdb.as_mut().unwrap().title = "Updated title".into();
    let plan = file_ops::organize_preview(
        &f.sources(),
        &[(changed.clone(), file_ops::suggested_name(&changed))],
    )
    .unwrap();
    assert!(
        plan.moves
            .iter()
            .any(|m| m.from == folder.join("poster.jpg")
                && m.to == f.root.join("Movies/2024 - Updated title/poster.jpg"))
    );
}

#[test]
fn movie_organization_refuses_existing_destinations_and_keeps_series_layout() {
    let f = Fixture::new();
    let original = f.root.join("Movies/release.mp4");
    fs::write(&original, b"movie").unwrap();
    let episode = f.root.join("Series/Show/Season 01/release.S01E01.mkv");
    fs::write(&episode, b"episode").unwrap();
    let mut db = f.db();
    let movie = f.matched(&mut db, Category::Movies, &original);
    let target_dir = f.root.join("Movies/2024 - A_ Title_");
    fs::create_dir(&target_dir).unwrap();
    fs::write(
        target_dir.join(file_ops::suggested_name(&movie)),
        b"existing",
    )
    .unwrap();
    assert!(
        file_ops::organize_preview(
            &f.sources(),
            &[(movie.clone(), file_ops::suggested_name(&movie))]
        )
        .is_err()
    );
    assert!(original.exists());
    let series = f.matched(&mut db, Category::Series, &episode);
    let plan = file_ops::organize_preview(
        &f.sources(),
        &[(series.clone(), file_ops::suggested_name(&series))],
    )
    .unwrap();
    assert_eq!(plan.moves[0].to.parent(), episode.parent());
}

#[test]
fn rename_sidecars_preserve_matches_and_undo_survives_restart() {
    let f = Fixture::new();
    let folder = f.root.join("Movies");
    let original = folder.join("release.mkv");
    for name in [
        "release.mkv",
        "release.en.forced.srt",
        "release.nfo",
        "release.jpg",
        "release-poster.jpg",
        "poster.jpg",
        "unrelated.srt",
    ] {
        fs::write(folder.join(name), name).unwrap();
    }
    let mut db = f.db();
    let video = f.matched(&mut db, Category::Movies, &original);
    assert_eq!(file_ops::suggested_name(&video), "A_ Title_ (2024).mkv");
    let plan =
        file_ops::rename_preview(&f.sources(), &[(video, "A Film (2024).mkv".into())]).unwrap();
    assert_eq!(plan.moves.len(), 5);
    assert!(original.exists());
    let id = file_ops::apply(&mut db, &plan).unwrap();
    drop(db);
    assert!(!original.exists());
    assert!(folder.join("A Film (2024).en.forced.srt").exists());
    assert!(folder.join("poster.jpg").exists() && folder.join("unrelated.srt").exists());
    let mut db = LibraryDb::open(&f.dbpath()).unwrap();
    assert_eq!(
        db.videos().unwrap()[0].path,
        folder.join("A Film (2024).mkv")
    );
    assert_eq!(db.videos().unwrap()[0].tmdb.as_ref().unwrap().id, 42);
    db.apply_scan(&f.sources(), &f.scan()).unwrap();
    assert!(db.videos().unwrap()[0].tmdb.is_some());
    let (saved_id, saved_plan) = file_ops::last_undo(&db).unwrap().unwrap();
    assert_eq!(id, saved_id);
    file_ops::undo(&mut db, id, &saved_plan).unwrap();
    assert!(original.exists() && folder.join("release.en.forced.srt").exists());
    assert_eq!(db.videos().unwrap()[0].path, original);
    assert!(db.videos().unwrap()[0].tmdb.is_some());
    assert!(file_ops::last_undo(&db).unwrap().is_none());
}

#[test]
fn episode_names_preserve_episode_numbers_and_handle_unrecognized_names() {
    let f = Fixture::new();
    let path = f
        .root
        .join("Series/Show/Season 01/Show.S01E02E03.1080p.mkv");
    fs::write(&path, b"ep").unwrap();
    let mut db = f.db();
    let mut video = f.matched(&mut db, Category::Series, &path);
    assert_eq!(
        file_ops::suggested_name(&video),
        "A_ Title_ - S01E02E03.mkv"
    );
    video.path = path.with_file_name("Show.1x03.mkv");
    assert_eq!(file_ops::suggested_name(&video), "A_ Title_ - S01E03.mkv");
    video.path = path.with_file_name("Unrecognized.mkv");
    assert_eq!(file_ops::suggested_name(&video), "Unrecognized.mkv");
}

#[test]
fn collisions_illegal_names_and_duplicate_batch_targets_change_nothing() {
    let f = Fixture::new();
    let folder = f.root.join("Movies");
    fs::write(folder.join("one.mp4"), b"one").unwrap();
    fs::write(folder.join("two.mp4"), b"two").unwrap();
    let db = f.db();
    let videos = db.videos().unwrap();
    for bad in [
        "../escape.mp4",
        "CON.mp4",
        "wrong.mkv",
        "trailing.mp4 ",
        "bad:name.mp4",
        "two.mp4",
    ] {
        assert!(
            file_ops::rename_preview(&f.sources(), &[(videos[0].clone(), bad.into())]).is_err(),
            "{bad}"
        );
    }
    assert!(
        file_ops::rename_preview(
            &f.sources(),
            &[
                (videos[0].clone(), "Same.mp4".into()),
                (videos[1].clone(), "same.mp4".into())
            ]
        )
        .is_err()
    );
    assert_eq!(fs::read(folder.join("one.mp4")).unwrap(), b"one");
    assert_eq!(fs::read(folder.join("two.mp4")).unwrap(), b"two");
}

#[test]
fn changed_files_or_new_collisions_after_preview_are_refused() {
    let f = Fixture::new();
    let original = f.root.join("Movies/original.mp4");
    fs::write(&original, b"movie").unwrap();
    let mut db = f.db();
    let plan = file_ops::rename_preview(
        &f.sources(),
        &[(db.videos().unwrap()[0].clone(), "New.mp4".into())],
    )
    .unwrap();
    fs::write(&original, b"changed content").unwrap();
    assert!(file_ops::apply(&mut db, &plan).is_err());
    assert!(original.exists());
    let plan = file_ops::rename_preview(
        &f.sources(),
        &[(db.videos().unwrap()[0].clone(), "New.mp4".into())],
    )
    .unwrap();
    fs::write(f.root.join("Movies/New.mp4"), b"existing").unwrap();
    assert!(file_ops::apply(&mut db, &plan).is_err());
    assert_eq!(
        fs::read(f.root.join("Movies/New.mp4")).unwrap(),
        b"existing"
    );
}

#[test]
fn cleanup_quarantines_only_selected_leftovers_and_restores_them() {
    let f = Fixture::new();
    let folder = f.root.join("Movies");
    for name in [
        "film.mkv",
        "film.srt",
        "film.nfo",
        "poster.jpg",
        "readme.txt",
        "active.part",
        "Thumbs.db",
        "desktop.ini",
        ".DS_Store",
        "release.sfv",
        "release.nzb",
    ] {
        fs::write(folder.join(name), name).unwrap();
    }
    let mut db = f.db();
    let mut plan = file_ops::cleanup_preview(&f.sources(), Category::Movies).unwrap();
    assert_eq!(plan.moves.len(), 5);
    plan.moves.retain(|m| {
        m.from.file_name().unwrap() == "Thumbs.db" || m.from.file_name().unwrap() == "release.sfv"
    });
    let id = file_ops::apply(&mut db, &plan).unwrap();
    assert!(!folder.join("Thumbs.db").exists());
    assert_eq!(
        fs::read(&plan.moves[0].to).unwrap().len() as u64,
        plan.moves[0].bytes
    );
    for name in [
        "film.mkv",
        "film.srt",
        "film.nfo",
        "poster.jpg",
        "readme.txt",
        "active.part",
        "desktop.ini",
        "release.nzb",
    ] {
        assert!(folder.join(name).exists(), "{name}");
    }
    assert_eq!(
        file_ops::cleanup_preview(&f.sources(), Category::Movies)
            .unwrap()
            .moves
            .len(),
        3
    );
    // Quarantine is excluded even if a video is placed in it manually.
    fs::write(
        folder.join(file_ops::QUARANTINE).join("ignored.mp4"),
        b"ignored",
    )
    .unwrap();
    assert_eq!(f.scan().videos.len(), 1);
    file_ops::undo(&mut db, id, &plan).unwrap();
    assert!(folder.join("Thumbs.db").exists() && folder.join("release.sfv").exists());
}

#[test]
fn database_failure_rolls_disk_changes_back() {
    let f = Fixture::new();
    let original = f.root.join("Movies/original.mp4");
    fs::write(&original, b"movie").unwrap();
    fs::write(f.root.join("Movies/original.srt"), b"subtitle").unwrap();
    let mut db = f.db();
    let plan = file_ops::rename_preview(
        &f.sources(),
        &[(db.videos().unwrap()[0].clone(), "New.mp4".into())],
    )
    .unwrap();
    let sql = rusqlite::Connection::open(f.dbpath()).unwrap();
    sql.execute_batch("CREATE TRIGGER reject_path BEFORE UPDATE OF path ON videos BEGIN SELECT RAISE(ABORT,'test database failure'); END;").unwrap();
    assert!(file_ops::apply(&mut db, &plan).is_err());
    assert!(original.exists() && f.root.join("Movies/original.srt").exists());
    assert!(!f.root.join("Movies/New.mp4").exists());
    assert_eq!(db.videos().unwrap()[0].path, original);
    assert!(file_ops::last_undo(&db).unwrap().is_none());
}

#[test]
fn interrupted_batch_can_be_recovered_and_blocks_rescans_until_then() {
    let f = Fixture::new();
    let original = f.root.join("Movies/original.mp4");
    fs::write(&original, b"movie").unwrap();
    fs::write(f.root.join("Movies/original.srt"), b"subtitle").unwrap();
    let mut db = f.db();
    let video = f.matched(&mut db, Category::Movies, &original);
    let plan = file_ops::rename_preview(&f.sources(), &[(video, "New.mp4".into())]).unwrap();
    let sql = rusqlite::Connection::open(f.dbpath()).unwrap();
    sql.execute(
        "INSERT INTO file_operations(plan_json,status) VALUES (?1,'pending')",
        [serde_json::to_string(&plan).unwrap()],
    )
    .unwrap();
    fs::rename(&plan.moves[0].from, &plan.moves[0].to).unwrap();
    drop(sql);
    drop(db);
    let mut db = LibraryDb::open(&f.dbpath()).unwrap();
    assert!(db.apply_scan(&f.sources(), &f.scan()).is_err());
    assert!(db.set_sources(&[]).is_err());
    let (id, loaded) = file_ops::last_undo(&db).unwrap().unwrap();
    file_ops::undo(&mut db, id, &loaded).unwrap();
    assert!(original.exists() && f.root.join("Movies/original.srt").exists());
    assert!(db.videos().unwrap()[0].tmdb.is_some());
    db.apply_scan(&f.sources(), &f.scan()).unwrap();
}

#[test]
fn undo_refuses_overwrites_and_modified_targets() {
    let f = Fixture::new();
    let original = f.root.join("Movies/original.mp4");
    fs::write(&original, b"movie").unwrap();
    let mut db = f.db();
    let plan = file_ops::rename_preview(
        &f.sources(),
        &[(db.videos().unwrap()[0].clone(), "New.mp4".into())],
    )
    .unwrap();
    let id = file_ops::apply(&mut db, &plan).unwrap();
    fs::write(&original, b"another file").unwrap();
    assert!(file_ops::undo(&mut db, id, &plan).is_err());
    assert_eq!(fs::read(&original).unwrap(), b"another file");
    fs::remove_file(&original).unwrap();
    fs::write(&plan.moves[0].to, b"modified destination").unwrap();
    assert!(file_ops::undo(&mut db, id, &plan).is_err());
}

#[test]
fn same_physical_video_in_two_categories_keeps_both_matches() {
    let f = Fixture::new();
    let original = f.root.join("Movies/original.mp4");
    fs::write(&original, b"movie").unwrap();
    let mut sources = f.sources();
    sources.push(Source {
        category: Category::Series,
        path: f.root.join("Movies"),
    });
    let mut db = LibraryDb::open(&f.dbpath()).unwrap();
    db.set_sources(&sources).unwrap();
    db.apply_scan(&sources, &scan(&sources, &AtomicBool::new(false), |_| {}))
        .unwrap();
    let first = f.matched(&mut db, Category::Movies, &original);
    f.matched(&mut db, Category::Series, &original);
    let plan = file_ops::rename_preview(&sources, &[(first, "New.mp4".into())]).unwrap();
    let id = file_ops::apply(&mut db, &plan).unwrap();
    for video in db.videos().unwrap() {
        assert_eq!(video.path, f.root.join("Movies/New.mp4"));
        assert!(video.tmdb.is_some());
    }
    file_ops::undo(&mut db, id, &plan).unwrap();
    for video in db.videos().unwrap() {
        assert_eq!(video.path, original);
        assert!(video.tmdb.is_some());
    }
}
