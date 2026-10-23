use media_shelf::{Category, Source, file_ops, nfo, scan, storage::LibraryDb};
use std::{fs, sync::atomic::AtomicBool};

fn fixture(xml: &str) -> (tempfile::TempDir, LibraryDb, Vec<Source>) {
    let dir = tempfile::tempdir().unwrap();
    fs::create_dir(dir.path().join("Movies")).unwrap();
    fs::write(dir.path().join("Movies/release.mp4"), b"video fixture").unwrap();
    fs::write(dir.path().join("Movies/movie.nfo"), xml).unwrap();
    let sources = vec![Source {
        category: Category::Movies,
        path: dir.path().canonicalize().unwrap().join("Movies"),
    }];
    let mut db = LibraryDb::open(&dir.path().join("library.sqlite3")).unwrap();
    db.set_sources(&sources).unwrap();
    db.apply_scan(&sources, &scan(&sources, &AtomicBool::new(false), |_| {}))
        .unwrap();
    (dir, db, sources)
}

#[test]
fn local_tmm_nfo_imports_persists_and_allows_rename_without_scraping() {
    let xml = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><movie><title>Fat Girl</title><originaltitle>À ma sœur !</originaltitle><year>2001</year><tmdbid>570</tmdbid><id>tt0243255</id><plot>A &amp; B</plot><ratings><rating name="themoviedb"><value>6.158</value><votes>262</votes></rating></ratings><runtime>86</runtime><certification>NR</certification><genre>Drama</genre><languages>French, Italian</languages><actor><name>Actor</name><role>Role</role><profile>https://www.themoviedb.org/person/7705</profile></actor><director>Director</director><fileinfo><streamdetails><video><codec>h264</codec></video></streamdetails></fileinfo></movie>"#;
    let (dir, mut db, sources) = fixture(xml);
    assert!(db.videos().unwrap()[0].tmdb.is_none());
    assert!(db.import_local_nfos().unwrap().is_empty());
    let video = db.videos().unwrap().remove(0);
    assert_eq!(video.tmdb.as_ref().unwrap().id, 570);
    let metadata = video.metadata.as_ref().unwrap();
    assert_eq!(metadata.identity.overview, "A & B");
    assert_eq!(metadata.imdb_id.as_deref(), Some("tt0243255"));
    assert_eq!(metadata.cast[0].tmdb_id, Some(7705));
    assert_eq!(metadata.languages, vec!["French", "Italian"]);
    assert!(metadata.nfo_written);
    let plan = file_ops::organize_preview(
        &sources,
        &[(video.clone(), file_ops::suggested_name(&video))],
    )
    .unwrap();
    let id = file_ops::apply(&mut db, &plan).unwrap();
    let destination = sources[0].path.join("2001 - Fat Girl/Fat Girl (2001).mp4");
    assert!(destination.exists());
    assert_eq!(
        fs::read_to_string(destination.with_file_name("movie.nfo")).unwrap(),
        xml
    );
    drop(db);
    let mut db = LibraryDb::open(&dir.path().join("library.sqlite3")).unwrap();
    assert_eq!(
        db.videos().unwrap()[0].tmdb.as_ref().unwrap().title,
        "Fat Girl"
    );
    db.import_local_nfos().unwrap();
    file_ops::undo(&mut db, id, &plan).unwrap();
    assert_eq!(
        fs::read_to_string(sources[0].path.join("movie.nfo")).unwrap(),
        xml
    );
    let mut changed = db.videos().unwrap()[0].metadata.clone().unwrap();
    changed.identity.title = "My custom title".into();
    db.save_scraped(&[(Category::Movies, video.path.clone(), changed)])
        .unwrap();
    db.import_local_nfos().unwrap();
    assert_eq!(
        db.videos().unwrap()[0].tmdb.as_ref().unwrap().title,
        "My custom title"
    );
}

#[test]
fn title_year_only_nfo_can_rename_without_inventing_provider_ids() {
    let (_dir, mut db, sources) =
        fixture("<movie><title>Local Movie</title><year>1999</year></movie>");
    db.import_local_nfos().unwrap();
    let video = db.videos().unwrap().remove(0);
    assert!(video.tmdb.is_none());
    assert_eq!(file_ops::suggested_name(&video), "Local Movie (1999).mp4");
    let plan = file_ops::organize_preview(
        &sources,
        &[(video.clone(), file_ops::suggested_name(&video))],
    )
    .unwrap();
    file_ops::apply(&mut db, &plan).unwrap();
    assert!(
        sources[0]
            .path
            .join("1999 - Local Movie/Local Movie (1999).mp4")
            .exists()
    );
    assert!(db.videos().unwrap()[0].tmdb.is_none());
}

#[test]
fn shared_generic_nfo_is_not_assigned_to_multiple_movies_and_invalid_xml_is_rejected() {
    let (_dir, mut db, sources) =
        fixture("<movie><title>One movie</title><year>2001</year><tmdbid>570</tmdbid></movie>");
    fs::write(sources[0].path.join("other.mp4"), b"other").unwrap();
    db.apply_scan(&sources, &scan(&sources, &AtomicBool::new(false), |_| {}))
        .unwrap();
    db.import_local_nfos().unwrap();
    assert!(db.videos().unwrap().iter().all(|v| v.tmdb.is_none()));
    assert!(
        nfo::parse("<!DOCTYPE movie [<!ENTITY x 'test'>]><movie><title>&x;</title></movie>")
            .is_err()
    );
    assert!(nfo::parse("<movie>broken").is_err());
    assert!(nfo::parse("<tvshow><title>Show</title></tvshow>").is_err());
}

#[test]
fn show_nfos_fill_cached_fields_preserve_edits_and_reload_explicitly() {
    let dir = tempfile::tempdir().unwrap();
    let show = dir
        .path()
        .canonicalize()
        .unwrap()
        .join("Series/Folder Name");
    fs::create_dir_all(show.join("Season 1")).unwrap();
    // A malformed episode NFO must not be read by a shallow show import.
    fs::write(show.join("Season 1/broken.nfo"), "broken").unwrap();
    let nfo = show.join("tvshow.nfo");
    let xml = "<tvshow><title>Local Show</title><year>2022</year><plot>Local plot</plot><rating>7.8</rating><genre>Comedy</genre><uniqueid type=\"tmdb\">123</uniqueid><uniqueid type=\"imdb\">tt123</uniqueid></tvshow>";
    fs::write(&nfo, xml).unwrap();
    let sources = vec![Source {
        category: Category::Series,
        path: dir.path().canonicalize().unwrap().join("Series"),
    }];
    let db_path = dir.path().join("library.sqlite3");
    let mut db = LibraryDb::open(&db_path).unwrap();
    db.set_sources(&sources).unwrap();
    let scan = media_shelf::index(&sources, &AtomicBool::new(false), |_| {});
    assert_eq!(scan.videos.len(), 1);
    assert!(scan.videos[0].is_folder);
    db.apply_scan(&sources, &scan).unwrap();
    assert!(db.import_local_nfos().unwrap().is_empty());
    let video = db.videos().unwrap().remove(0);
    let added = video.added_date.clone();
    let mut data = video.metadata.unwrap();
    assert_eq!(data.year, Some(2022));
    assert_eq!(data.identity.title, "Local Show");
    assert_eq!(data.rating, Some(7.8));
    assert_eq!(data.genres, ["Comedy"]);
    assert_eq!(data.imdb_id.as_deref(), Some("tt123"));
    assert_eq!(video.tmdb.unwrap().id, 123);
    data.identity.title = "Edited title".into();
    data.year = None;
    db.save_scraped(&[(Category::Series, show.clone(), data)])
        .unwrap();
    db.import_local_nfos().unwrap();
    let video = db.videos().unwrap().remove(0);
    assert_eq!(video.metadata.as_ref().unwrap().year, Some(2022));
    assert_eq!(video.tmdb.unwrap().title, "Edited title");
    fs::write(
        &nfo,
        xml.replace("2022", "2023")
            .replace("Local Show", "Updated Show"),
    )
    .unwrap();
    let expected_nfo = fs::read(&nfo).unwrap();
    assert_eq!(db.reload_local_nfos(&[show.clone()]).unwrap(), (1, vec![]));
    drop(db);
    let mut db = LibraryDb::open(&db_path).unwrap();
    let video = db.videos().unwrap().remove(0);
    assert_eq!(video.tmdb.unwrap().title, "Updated Show");
    assert_eq!(video.metadata.unwrap().year, Some(2023));
    assert_eq!(video.added_date, added);
    assert_eq!(
        fs::read(&nfo).unwrap(),
        expected_nfo,
        "Import never rewrites the NFO"
    );
    fs::write(&nfo, xml.replace(">123<", ">456<")).unwrap();
    let (count, warnings) = db.reload_local_nfos(&[show.clone()]).unwrap();
    assert_eq!(count, 0);
    assert!(warnings[0].contains("conflicts"));
    assert_eq!(db.videos().unwrap()[0].tmdb.as_ref().unwrap().id, 123);
    fs::write(&nfo, "<tvshow>broken").unwrap();
    assert_eq!(db.import_local_nfos().unwrap().len(), 1);
    assert_eq!(
        db.videos().unwrap()[0].metadata.as_ref().unwrap().year,
        Some(2023)
    );
}

#[test]
fn show_without_provider_id_imports_and_survives_rescan_without_a_false_match() {
    let dir = tempfile::tempdir().unwrap();
    let show = dir.path().canonicalize().unwrap().join("Series/Show");
    fs::create_dir_all(&show).unwrap();
    fs::write(show.join("tvshow.nfo"), "<tvshow><title>Offline Show</title><premiered>1999-03-28</premiered><ratings><rating max=\"5\"><value>4</value></rating></ratings></tvshow>").unwrap();
    let sources = vec![Source {
        category: Category::Series,
        path: dir.path().canonicalize().unwrap().join("Series"),
    }];
    let mut db = LibraryDb::open(&dir.path().join("db.sqlite3")).unwrap();
    db.set_sources(&sources).unwrap();
    for _ in 0..2 {
        db.apply_scan(
            &sources,
            &media_shelf::index(&sources, &AtomicBool::new(false), |_| {}),
        )
        .unwrap();
        assert!(db.import_local_nfos().unwrap().is_empty());
        let video = db.videos().unwrap().remove(0);
        assert!(video.tmdb.is_none());
        let data = video.metadata.unwrap();
        assert_eq!(data.identity.title, "Offline Show");
        assert_eq!(data.year, Some(1999));
        assert_eq!(data.rating, Some(8.0));
    }
}

#[test]
fn show_import_keeps_saved_match_when_local_nfo_has_no_provider_id() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    let show = root.join("Show");
    fs::create_dir(&show).unwrap();
    fs::write(
        show.join("tvshow.nfo"),
        "<tvshow><title>NFO title</title><year>2020</year><plot>Plot</plot></tvshow>",
    )
    .unwrap();
    let sources = vec![Source {
        category: Category::Series,
        path: root,
    }];
    let mut db = LibraryDb::open(&dir.path().join("db.sqlite3")).unwrap();
    db.set_sources(&sources).unwrap();
    db.apply_scan(
        &sources,
        &media_shelf::index(&sources, &AtomicBool::new(false), |_| {}),
    )
    .unwrap();
    let matched = media_shelf::tmdb::TmdbMatch {
        category: Category::Series,
        id: 123,
        title: "Saved match".into(),
        original_title: "Saved match".into(),
        release_date: None,
        overview: String::new(),
    };
    db.save_match(Category::Series, &show, &matched).unwrap();
    assert!(db.import_local_nfos().unwrap().is_empty());
    let video = db
        .videos()
        .unwrap()
        .into_iter()
        .find(|v| v.path == show)
        .unwrap();
    assert_eq!(video.tmdb.unwrap().title, "Saved match");
    let data = video.metadata.unwrap();
    assert_eq!(data.identity.id, 123);
    assert_eq!(data.year, Some(2020));
    assert_eq!(data.identity.overview, "Plot");
}
