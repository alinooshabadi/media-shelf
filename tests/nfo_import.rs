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
