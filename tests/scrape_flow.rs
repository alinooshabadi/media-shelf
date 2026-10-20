use media_shelf::{
    Category, Source, file_ops,
    metadata::{self, Field, Metadata, Person, ScrapeOptions},
    scan,
    storage::LibraryDb,
};
use serde_json::json;
use std::{fs, sync::atomic::AtomicBool};

fn movie() -> Metadata {
    metadata::parse_details(&json!({"id":42,"title":"A & B <Film>","original_title":"Original","release_date":"1995-11-10","overview":"A plot & more","tagline":"A tagline","runtime":90,"vote_average":6.3,"vote_count":4218,"genres":[{"name":"Comedy"}],"spoken_languages":[{"english_name":"English"}],"production_countries":[{"name":"United States"}],"production_companies":[{"name":"Studio"}],"belongs_to_collection":{"name":"A Collection"},"keywords":{"keywords":[{"name":"adventure"}]},"credits":{"cast":[{"name":"Actor","character":"Role"}],"crew":[{"name":"Director","job":"Director"},{"name":"Writer","job":"Screenplay"},{"name":"Producer","job":"Producer"}]},"videos":{"results":[{"site":"YouTube","type":"Trailer","key":"abc-123"}]},"release_dates":{"results":[{"iso_3166_1":"US","release_dates":[{"certification":"PG-13"}]},{"iso_3166_1":"DE","release_dates":[{"certification":"12"}]}]},"poster_path":"/poster.jpg","backdrop_path":"/backdrop.jpg","external_ids":{"imdb_id":"tt123"}}),Category::Movies,42,"de-DE").unwrap()
}

#[test]
fn candidate_preview_maps_full_details_and_localized_certification() {
    let m = movie();
    assert_eq!(m.year, Some(1995));
    assert_eq!(m.runtime, Some(90));
    assert_eq!(m.certification.as_deref(), Some("12"));
    assert_eq!(m.rating, Some(6.3));
    assert_eq!(m.cast[0].role, "Role");
    assert_eq!(m.crew.len(), 3);
    assert_eq!(m.genres, vec!["Comedy"]);
    assert_eq!(m.tags, vec!["adventure"]);
    assert_eq!(m.imdb_id.as_deref(), Some("tt123"));
    assert_eq!(m.collection.as_deref(), Some("A Collection"));
    assert!(
        metadata::parse_details(
            &json!({"id":5,"title":"Wrong"}),
            Category::Movies,
            42,
            "en-US"
        )
        .is_err()
    );
}

#[test]
fn selected_fields_merge_without_touching_unchecked_data() {
    let new = movie();
    let mut old = new.clone();
    old.identity.title = "My custom title".into();
    old.identity.overview = "My custom plot".into();
    old.rating = Some(9.0);
    old.tagline = Some("Custom".into());
    old.cast = vec![Person {
        name: "Custom actor".into(),
        role: "Custom role".into(),
        ..Default::default()
    }];
    let opts = ScrapeOptions {
        fields: vec![Field::Rating, Field::Genres],
        only_missing: false,
        write_nfo: false,
        replace_nfo: false,
    };
    let m = metadata::merge(Some(&old), old.identity.clone(), &new, &opts);
    assert_eq!(m.rating, Some(6.3));
    assert_eq!(m.identity.title, "My custom title");
    assert_eq!(m.identity.overview, "My custom plot");
    assert_eq!(m.cast, old.cast);
    assert_eq!(m.tagline, old.tagline);
    let opts = ScrapeOptions {
        fields: vec![Field::Title, Field::Plot],
        ..opts
    };
    let m = metadata::merge(Some(&old), old.identity.clone(), &new, &opts);
    assert_eq!(m.identity.title, new.identity.title);
    assert_eq!(m.identity.overview, new.identity.overview);
    assert_eq!(m.rating, Some(9.0));
}

#[test]
fn fill_missing_preserves_custom_values_and_crew_categories_are_independent() {
    let new = movie();
    let mut old = new.clone();
    old.tagline = None;
    old.identity.title = "Custom".into();
    old.crew = vec![Person {
        name: "Custom Director".into(),
        role: "Director".into(),
        ..Default::default()
    }];
    let opts = ScrapeOptions {
        only_missing: true,
        ..Default::default()
    };
    let m = metadata::merge(Some(&old), old.identity.clone(), &new, &opts);
    assert_eq!(m.identity.title, "Custom");
    assert_eq!(m.tagline, new.tagline);
    assert!(m.crew.contains(&old.crew[0]));
    assert!(m.crew.iter().any(|p| p.role == "Screenplay"));
    let opts = ScrapeOptions {
        fields: vec![Field::Producers],
        only_missing: false,
        ..opts
    };
    let m = metadata::merge(Some(&old), old.identity.clone(), &new, &opts);
    assert!(m.crew.contains(&old.crew[0]));
    assert!(m.crew.iter().any(|p| p.role == "Producer"));
    assert!(!m.crew.iter().any(|p| p.role == "Screenplay"));
}

#[test]
fn movie_nfo_follows_supplied_tmm_structure() {
    let mut data = movie();
    data.original_filename = Some("Original.Release.1995.mkv".into());
    data.cast[0].tmdb_id = Some(7705);
    data.cast[0].profile_path = Some("/person.jpg".into());
    let text = metadata::xml(&data);
    assert!(text.starts_with("<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>"));
    let doc = roxmltree::Document::parse(&text).unwrap();
    let root = doc.root_element();
    for name in [
        "title",
        "originaltitle",
        "sorttitle",
        "epbookmark",
        "year",
        "ratings",
        "userrating",
        "top250",
        "set",
        "plot",
        "outline",
        "tagline",
        "runtime",
        "thumb",
        "fanart",
        "mpaa",
        "certification",
        "id",
        "tmdbid",
        "tmdbCollectionId",
        "uniqueid",
        "country",
        "status",
        "code",
        "premiered",
        "watched",
        "playcount",
        "lastplayed",
        "genre",
        "studio",
        "credits",
        "director",
        "tag",
        "actor",
        "producer",
        "trailer",
        "languages",
        "dateadded",
        "fileinfo",
        "source",
        "edition",
        "original_filename",
        "user_note",
    ] {
        assert!(
            root.children().any(|n| n.has_tag_name(name)),
            "Missing sample tag: {name}"
        );
    }
    let field = |name| {
        root.children()
            .find(|n| n.has_tag_name(name))
            .unwrap()
            .text()
    };
    assert_eq!(field("outline"), field("plot"));
    assert_eq!(field("certification"), field("mpaa"));
    assert_eq!(field("tmdbid"), Some("42"));
    assert_eq!(field("id"), Some("tt123"));
    assert_eq!(
        field("original_filename"),
        Some("Original.Release.1995.mkv")
    );
    assert_eq!(
        field("trailer"),
        Some("plugin://plugin.video.youtube/?action=play_video&videoid=abc-123")
    );
    assert_eq!(
        doc.descendants()
            .find(|n| n.has_tag_name("rating"))
            .unwrap()
            .attribute("name"),
        Some("themoviedb")
    );
    let imdb = root
        .children()
        .find(|n| n.has_tag_name("uniqueid") && n.attribute("type") == Some("imdb"))
        .unwrap();
    assert_eq!(imdb.attribute("default"), Some("true"));
    let actor = root.children().find(|n| n.has_tag_name("actor")).unwrap();
    assert_eq!(
        actor
            .children()
            .find(|n| n.has_tag_name("profile"))
            .unwrap()
            .text(),
        Some("https://www.themoviedb.org/person/7705")
    );
    assert!(
        actor
            .children()
            .find(|n| n.has_tag_name("thumb"))
            .unwrap()
            .text()
            .unwrap()
            .contains("/h632/person.jpg")
    );
    assert!(
        doc.descendants()
            .filter(|n| n.has_tag_name("codec"))
            .all(|n| n.text().is_none())
    );
    data.imdb_id = None;
    let fallback = metadata::xml(&data);
    let fallback = roxmltree::Document::parse(&fallback).unwrap();
    assert_eq!(
        fallback
            .descendants()
            .find(|n| n.has_tag_name("uniqueid"))
            .unwrap()
            .attribute("default"),
        Some("true")
    );
    let legacy = r#"{"name":"Old actor","role":"Old role"}"#;
    assert_eq!(
        serde_json::from_str::<Person>(legacy).unwrap().tmdb_id,
        None
    );
}

#[test]
fn output_files_require_explicit_replacement_and_create_backups() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("film.nfo");
    let backup = temp.path().join("backups");
    fs::write(&path, b"original user metadata").unwrap();
    let output = metadata::xml(&movie());
    assert!(metadata::write_output(&path, output.as_bytes(), false, &backup).is_err());
    assert_eq!(fs::read(&path).unwrap(), b"original user metadata");
    assert!(!backup.exists());
    metadata::write_output(&path, output.as_bytes(), true, &backup).unwrap();
    let original_backup = fs::read_dir(&backup)
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    assert_eq!(
        fs::read(original_backup).unwrap(),
        b"original user metadata"
    );
    let text = fs::read_to_string(&path).unwrap();
    let doc = roxmltree::Document::parse(&text).unwrap();
    assert_eq!(doc.root_element().tag_name().name(), "movie");
    assert_eq!(
        doc.descendants()
            .find(|n| n.has_tag_name("title"))
            .unwrap()
            .text(),
        Some("A & B <Film>")
    );
    assert!(metadata::image_url("/../../escape.jpg", false).is_none());
    assert!(metadata::image_url("https://elsewhere/image.jpg", false).is_none());
}

fn setup() -> (tempfile::TempDir, LibraryDb, Vec<Source>) {
    let temp = tempfile::tempdir().unwrap();
    for folder in ["Movies", "Series/Show/Season 01", "Series/Other/Season 01"] {
        fs::create_dir_all(temp.path().join(folder)).unwrap();
    }
    for file in [
        "Movies/raw.mkv",
        "Series/Show/Season 01/Show.S01E01.mkv",
        "Series/Show/Season 01/Show.S01E02.mkv",
        "Series/Other/Season 01/Other.S01E01.mkv",
    ] {
        fs::write(temp.path().join(file), b"video").unwrap();
    }
    let root = temp.path().canonicalize().unwrap();
    let sources = vec![
        Source {
            category: Category::Movies,
            path: root.join("Movies"),
        },
        Source {
            category: Category::Series,
            path: root.join("Series"),
        },
    ];
    let mut db = LibraryDb::open(&root.join("library.sqlite3")).unwrap();
    db.set_sources(&sources).unwrap();
    db.apply_scan(&sources, &scan(&sources, &AtomicBool::new(false), |_| {}))
        .unwrap();
    (temp, db, sources)
}

#[test]
fn organized_movie_rescrape_keeps_standard_nfo_name() {
    let (_temp, mut db, sources) = setup();
    let video = db
        .videos()
        .unwrap()
        .into_iter()
        .find(|v| v.category == Category::Movies)
        .unwrap();
    let data = movie();
    db.save_match(Category::Movies, &video.path, &data.identity)
        .unwrap();
    let video = db
        .videos()
        .unwrap()
        .into_iter()
        .find(|v| v.category == Category::Movies)
        .unwrap();
    let plan = file_ops::organize_preview(
        &sources,
        &[(video.clone(), file_ops::suggested_name(&video))],
    )
    .unwrap();
    file_ops::apply(&mut db, &plan).unwrap();
    let current = db
        .videos()
        .unwrap()
        .into_iter()
        .find(|v| v.category == Category::Movies)
        .unwrap();
    let options = ScrapeOptions {
        fields: vec![Field::Title],
        write_nfo: true,
        replace_nfo: false,
        only_missing: false,
    };
    let (_, warnings) = metadata::commit_scrape(&mut db, &current, &data, &options, false).unwrap();
    assert!(warnings.is_empty(), "{warnings:?}");
    assert!(current.path.with_file_name("movie.nfo").exists());
    assert!(!current.path.with_extension("nfo").exists());
}

#[test]
fn confirmed_scrape_persists_and_survives_rescan_rename_and_undo() {
    let (_temp, mut db, sources) = setup();
    let video = db
        .videos()
        .unwrap()
        .into_iter()
        .find(|v| v.category == Category::Movies)
        .unwrap();
    let data = movie();
    // Loading preview details alone has no effect on the library or sidecar files.
    assert!(db.videos().unwrap().iter().all(|v| v.metadata.is_none()));
    assert!(!video.path.with_extension("nfo").exists());
    let options = ScrapeOptions {
        fields: vec![
            Field::Title,
            Field::Year,
            Field::Plot,
            Field::Actors,
            Field::Rating,
        ],
        write_nfo: true,
        ..Default::default()
    };
    let (count, warnings) =
        metadata::commit_scrape(&mut db, &video, &data, &options, false).unwrap();
    assert_eq!(count, 1);
    assert!(warnings.is_empty());
    let video = db
        .videos()
        .unwrap()
        .into_iter()
        .find(|v| v.category == Category::Movies)
        .unwrap();
    let stored = video.metadata.as_ref().unwrap();
    assert!(stored.poster.is_none());
    assert_eq!(stored.rating, Some(6.3));
    assert!(stored.nfo_written);
    assert!(video.path.with_extension("nfo").exists());
    db.apply_scan(&sources, &scan(&sources, &AtomicBool::new(false), |_| {}))
        .unwrap();
    assert_eq!(db.videos().unwrap()[0].metadata.as_ref(), Some(stored));
    let plan = file_ops::rename_preview(&sources, &[(video, "New.mkv".into())]).unwrap();
    let id = file_ops::apply(&mut db, &plan).unwrap();
    assert!(db.videos().unwrap()[0].metadata.is_some());
    file_ops::undo(&mut db, id, &plan).unwrap();
    assert!(db.videos().unwrap()[0].metadata.is_some());
    db.clear_match(Category::Movies, &sources[0].path.join("raw.mkv"))
        .unwrap();
    assert!(db.videos().unwrap()[0].metadata.is_none());
}

#[test]
fn show_scrape_groups_episodes_and_writes_show_nfo_without_episode_identity() {
    let (_temp, mut db, sources) = setup();
    let video = db
        .videos()
        .unwrap()
        .into_iter()
        .find(|v| v.path.to_string_lossy().contains("Show.S01E01"))
        .unwrap();
    let data=metadata::parse_details(&json!({"id":77,"name":"A Show","original_name":"Original Show","first_air_date":"2020-01-01","overview":"Show overview","episode_run_time":[45],"created_by":[{"name":"Creator"}],"credits":{"cast":[]},"content_ratings":{"results":[{"iso_3166_1":"US","rating":"TV-14"}]}}),Category::Series,77,"en-US").unwrap();
    assert_eq!(data.runtime, Some(45));
    assert_eq!(data.certification.as_deref(), Some("TV-14"));
    let options = ScrapeOptions {
        fields: vec![Field::Title, Field::Year, Field::Plot, Field::Directors],
        write_nfo: true,
        ..Default::default()
    };
    let (count, warnings) =
        metadata::commit_scrape(&mut db, &video, &data, &options, true).unwrap();
    assert_eq!(count, 2);
    assert!(warnings.is_empty());
    let videos = db.videos().unwrap();
    assert_eq!(videos.iter().filter(|v| v.metadata.is_some()).count(), 2);
    assert!(
        videos.iter().filter(|v| v.metadata.is_some()).all(|v| v
            .metadata
            .as_ref()
            .unwrap()
            .nfo_written)
    );
    let show = sources[1].path.join("Show");
    let text = fs::read_to_string(show.join("tvshow.nfo")).unwrap();
    assert_eq!(
        roxmltree::Document::parse(&text)
            .unwrap()
            .root_element()
            .tag_name()
            .name(),
        "tvshow"
    );
    assert!(!video.path.with_extension("nfo").exists());
    let (_, warnings) = metadata::commit_scrape(&mut db, &video, &data, &options, true).unwrap();
    assert!(!warnings.is_empty());
    assert!(
        db.videos()
            .unwrap()
            .iter()
            .filter(|v| v.metadata.is_some())
            .all(|v| !v.metadata.as_ref().unwrap().nfo_written)
    );
}

#[test]
fn database_upgrade_preserves_old_matches_and_json_can_be_reopened() {
    let (temp, mut db, sources) = setup();
    let video = db.videos().unwrap()[0].clone();
    let data = movie();
    let options = ScrapeOptions {
        fields: vec![Field::Title, Field::Plot],
        write_nfo: false,
        ..Default::default()
    };
    metadata::commit_scrape(&mut db, &video, &data, &options, false).unwrap();
    let path = temp.path().canonicalize().unwrap().join("library.sqlite3");
    drop(db);
    let mut db = LibraryDb::open(&path).unwrap();
    assert!(db.videos().unwrap()[0].metadata.is_some());
    db.set_sources(&sources[1..]).unwrap();
    let sql = rusqlite::Connection::open(path).unwrap();
    assert_eq!(
        sql.query_row("SELECT count(*) FROM scraped_metadata", [], |r| r
            .get::<_, i64>(0))
            .unwrap(),
        0
    );
}
