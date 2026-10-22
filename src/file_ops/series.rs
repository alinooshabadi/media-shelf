use super::*;
use std::{collections::BTreeMap, io::Read, sync::LazyLock};

static EPISODE: LazyLock<regex::Regex> = LazyLock::new(|| {
    regex::Regex::new(r"(?i)(?:^|[ ._-])(?:s(\d{1,3})e(\d{1,3})|(\d{1,3})x(\d{1,3}))").unwrap()
});
static SEASON: LazyLock<regex::Regex> =
    LazyLock::new(|| regex::Regex::new(r"(?i)^season[ ._-]*(\d{1,3})$").unwrap());

fn episode(path: &Path) -> Option<(u32, u32, usize)> {
    let stem = path.file_stem()?.to_str()?;
    let captures = EPISODE.captures(stem)?;
    let end = captures.get(0)?.end();
    let tail = &stem[end..];
    // shortcut: multi-episode files keep their names until explicit multi-episode title rules exist.
    if tail.starts_with(|c: char| c.is_ascii_alphanumeric())
        || tail.strip_prefix('-').is_some_and(|t| {
            t.starts_with(|c: char| c.eq_ignore_ascii_case(&'e') || c.is_ascii_digit())
        })
    {
        return None;
    }
    Some((
        captures
            .get(1)
            .or_else(|| captures.get(3))?
            .as_str()
            .parse()
            .ok()?,
        captures
            .get(2)
            .or_else(|| captures.get(4))?
            .as_str()
            .parse()
            .ok()?,
        end,
    ))
}

fn local_title(path: &Path, season: u32, number: u32, end: usize) -> DbResult<Option<String>> {
    match fs::File::open(path.with_extension("nfo")) {
        Ok(file) => {
            let mut bytes = vec![];
            file.take(4_000_001).read_to_end(&mut bytes)?;
            if bytes.len() > 4_000_000 {
                return Err("Episode NFO is too large".into());
            }
            if let Ok(text) = std::str::from_utf8(&bytes)
                && let Ok(doc) = roxmltree::Document::parse(text)
            {
                let root = doc.root_element();
                let value = |tag| {
                    root.children()
                        .find(|n| n.has_tag_name(tag))
                        .and_then(|n| n.text())
                };
                if root.has_tag_name("episodedetails")
                    && value("season").is_none_or(|s| s.parse().ok() == Some(season))
                    && value("episode").is_none_or(|s| s.parse().ok() == Some(number))
                    && let Some(title) = value("title").filter(|s| !s.trim().is_empty())
                {
                    return Ok(Some(title.trim().to_string()));
                }
            }
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(e.into()),
    }
    let title = path
        .file_stem()
        .and_then(|s| s.to_str())
        .and_then(|s| s[end..].strip_prefix(" - "))
        .map(str::trim)
        .filter(|s| !s.is_empty());
    let technical = regex::Regex::new(
        r"(?i)^(?:web[- .]?dl|webrip|bluray|blu-ray|\d{3,4}p|[xh]26[45])(?:$|[ ._-])",
    )
    .unwrap();
    Ok(title.filter(|s| !technical.is_match(s)).map(str::to_owned))
}

fn destination(root: &Path, target: &Path, file: &Path) -> DbResult<PathBuf> {
    let relative = file.strip_prefix(root)?;
    let mut parts = relative.components();
    let first = parts.next().ok_or("Missing show-relative filename")?;
    let first_name = first.as_os_str().to_string_lossy();
    let base = if relative.components().count() > 1 {
        SEASON
            .captures(&first_name)
            .map(|c| format!("Season {}", c[1].parse::<u32>().unwrap()))
    } else {
        None
    };
    let output = target.join(
        base.map(PathBuf::from)
            .unwrap_or_else(|| first.as_os_str().into()),
    );
    Ok(if parts.as_path().as_os_str().is_empty() {
        output
    } else {
        output.join(parts.as_path())
    })
}

pub(super) fn append(
    plan: &mut OperationPlan,
    seen: &mut HashSet<PathBuf>,
    show: &Video,
    mut fetch: impl FnMut(u32) -> Result<BTreeMap<u32, String>, String>,
) -> DbResult<()> {
    if show.category != Category::Series || !show.is_folder {
        return Err("Select an indexed series folder to organize.".into());
    }
    let matched = show
        .tmdb
        .as_ref()
        .filter(|m| m.category == Category::Series)
        .ok_or("Search & scrape this show before renaming it.")?;
    let year = show
        .metadata
        .as_ref()
        .and_then(|m| m.year)
        .map(|y| y.to_string())
        .unwrap_or_else(|| matched.year().to_string());
    if year == "Unknown year" {
        return Err("Show year is missing. Match the show before renaming.".into());
    }
    let title = sanitize_title(&matched.title);
    let folder = format!("{title} ({year})");
    validate_name(&folder)?;
    let root = plan
        .sources
        .iter()
        .filter(|s| s.category == Category::Series && show.path.starts_with(&s.path))
        .max_by_key(|s| s.path.components().count())
        .ok_or("Show is outside its sources")?;
    if show.path.parent() != Some(root.path.as_path()) {
        return Err("Select a first-level show folder to rename.".into());
    }
    let target = root.path.join(folder);
    if target != show.path && fs::symlink_metadata(&target).is_ok() {
        return Err("The destination show folder already exists; shows will not be merged.".into());
    }
    let mut files = vec![];
    for entry in WalkDir::new(&show.path)
        .min_depth(1)
        .follow_links(false)
        .follow_root_links(false)
    {
        let entry = entry?;
        if entry.file_type().is_symlink() {
            return Err("Show contains a symbolic link; resolve it before renaming.".into());
        }
        if entry.file_type().is_file() {
            files.push(entry.into_path());
        }
    }
    files.sort();
    if files.is_empty() {
        return Err("This show folder has no files to organize.".into());
    }
    let mut seasons = BTreeMap::new();
    let mut episodes = BTreeMap::new();
    for file in files
        .iter()
        .filter(|p| crate::is_video(p) && !crate::identification::sample_file(p))
    {
        let Some((season, number, end)) = episode(file) else {
            plan.notes.push(format!(
                "Unrecognized episode numbering; filename kept: {}",
                file.display()
            ));
            continue;
        };
        let episode_title = if let Some(title) = local_title(file, season, number, end)? {
            title
        } else {
            if !seasons.contains_key(&season) {
                seasons.insert(season, fetch(season)?);
            }
            seasons[&season]
                .get(&number)
                .filter(|s| !s.trim().is_empty())
                .cloned()
                .ok_or_else(|| {
                    format!(
                        "Episode title missing for S{season:02}E{number:02}; nothing was renamed."
                    )
                })?
        };
        let name = format!(
            "{title} - S{season:02}E{number:02} - {}.{}",
            sanitize_title(&episode_title),
            file.extension().unwrap_or_default().to_string_lossy()
        );
        validate_name(&name)?;
        episodes.insert(
            file.clone(),
            target.join(format!("Season {season}")).join(name),
        );
    }
    for file in files {
        let mut output = destination(&show.path, &target, &file)?;
        if let Some(renamed) = episodes.get(&file) {
            output = renamed.clone();
        } else {
            let filename = file.file_name().unwrap_or_default().to_string_lossy();
            for (episode, renamed) in &episodes {
                if episode.parent() != file.parent() {
                    continue;
                }
                let stem = episode
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .ok_or("Invalid episode filename")?;
                if let Some(suffix) = filename
                    .strip_prefix(stem)
                    .filter(|s| s.starts_with('.') || s.to_ascii_lowercase().starts_with("-thumb."))
                {
                    output = renamed.with_file_name(format!(
                        "{}{suffix}",
                        renamed.file_stem().unwrap().to_string_lossy()
                    ));
                    break;
                }
            }
        }
        if file != output && !same_existing_path(&file, &output) {
            add_move(plan, seen, &file, &output, crate::is_video(&file))?;
        }
    }
    plan.folders.push((show.path.clone(), target));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exclusive_move_refuses_to_replace_an_existing_file() {
        let temp = tempfile::tempdir().unwrap();
        let from = temp.path().join("source.mkv");
        let to = temp.path().join("destination.mkv");
        fs::write(&from, b"source").unwrap();
        fs::write(&to, b"existing").unwrap();
        assert!(move_no_replace(&from, &to).is_err());
        assert_eq!(fs::read(&to).unwrap(), b"existing");
        assert_eq!(fs::read(&from).unwrap(), b"source");
    }

    fn fixture() -> (tempfile::TempDir, LibraryDb, Video) {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().canonicalize().unwrap();
        let folder = root.join("release-name");
        fs::create_dir_all(folder.join("Season 01")).unwrap();
        for (name, content) in [
            ("Season 01/release.S01E01.1080p.mkv", "first episode"),
            ("Season 01/release.S01E01.1080p.fa.srt", "subtitle"),
            ("Season 01/release.S01E01.1080p-thumb.jpg", "thumbnail"),
            ("Season 01/release.1x02.mp4", "second episode"),
            (
                "Season 01/release.1x02.nfo",
                "<episodedetails><title>Local Title</title><season>1</season><episode>2</episode></episodedetails>",
            ),
            ("Season 01/release.S01E03.mkv", "third episode"),
            ("poster.jpg", "poster"),
            ("tvshow.nfo", "show nfo"),
            ("keep.part", "unfinished download"),
        ] {
            fs::write(folder.join(name), content).unwrap();
        }
        let sources = vec![Source {
            category: Category::Series,
            path: root.clone(),
        }];
        let mut db = LibraryDb::open(&root.join("library.sqlite3")).unwrap();
        db.set_sources(&sources).unwrap();
        db.apply_scan(
            &sources,
            &crate::index(&sources, &std::sync::atomic::AtomicBool::new(false), |_| {}),
        )
        .unwrap();
        db.save_match(
            Category::Series,
            &folder,
            &crate::tmdb::TmdbMatch {
                category: Category::Series,
                id: 42,
                title: "A Show".into(),
                original_title: "A Show".into(),
                release_date: Some("2020-01-01".into()),
                overview: String::new(),
            },
        )
        .unwrap();
        let show = db.videos().unwrap().remove(0);
        (temp, db, show)
    }

    fn plan(db: &LibraryDb, show: &Video) -> OperationPlan {
        let mut plan = OperationPlan {
            kind: "organize".into(),
            sources: db.sources().unwrap(),
            moves: vec![],
            folders: vec![],
            notes: vec![],
        };
        let mut requests = 0;
        append(&mut plan, &mut HashSet::new(), show, |season| {
            assert_eq!(season, 1);
            requests += 1;
            Ok(BTreeMap::from([
                (1, "Pilot".into()),
                (3, "Third Episode".into()),
            ]))
        })
        .unwrap();
        assert_eq!(
            requests, 1,
            "Fetch each season once; use local NFO titles first"
        );
        validate_plan(&plan, false).unwrap();
        plan
    }

    #[test]
    fn organization_matches_nas_layout_moves_sidecars_and_undo_survives_restart() {
        let (_temp, mut db, show) = fixture();
        let root = db.sources().unwrap()[0].path.clone();
        let plan = plan(&db, &show);
        let id = apply(&mut db, &plan).unwrap();
        let new = root.join("A Show (2020)");
        assert_eq!(
            fs::read(new.join("Season 1/A Show - S01E01 - Pilot.mkv")).unwrap(),
            b"first episode"
        );
        assert_eq!(
            fs::read(new.join("Season 1/A Show - S01E01 - Pilot.fa.srt")).unwrap(),
            b"subtitle"
        );
        assert!(
            new.join("Season 1/A Show - S01E01 - Pilot-thumb.jpg")
                .exists()
        );
        assert!(
            new.join("Season 1/A Show - S01E02 - Local Title.mp4")
                .exists()
        );
        assert!(
            new.join("Season 1/A Show - S01E02 - Local Title.nfo")
                .exists()
        );
        assert!(
            new.join("Season 1/A Show - S01E03 - Third Episode.mkv")
                .exists()
        );
        assert_eq!(
            fs::read(new.join("keep.part")).unwrap(),
            b"unfinished download"
        );
        assert!(new.join("poster.jpg").exists() && new.join("tvshow.nfo").exists());
        assert!(!show.path.exists());
        let videos = db.videos().unwrap();
        assert_eq!(videos.len(), 1);
        assert_eq!(videos[0].path, new);
        assert_eq!(videos[0].tmdb, show.tmdb);
        drop(db);
        let mut db = LibraryDb::open(&root.join("library.sqlite3")).unwrap();
        let (saved_id, saved) = last_undo(&db).unwrap().unwrap();
        assert_eq!(id, saved_id);
        undo(&mut db, saved_id, &saved).unwrap();
        for item in &plan.moves {
            assert!(item.from.exists() && !item.to.exists());
        }
        assert_eq!(db.videos().unwrap()[0].path, show.path);
        assert!(!new.exists());
    }

    #[test]
    fn missing_titles_and_duplicate_episode_destinations_change_nothing() {
        let (_temp, db, show) = fixture();
        let mut plan = OperationPlan {
            kind: "organize".into(),
            sources: db.sources().unwrap(),
            moves: vec![],
            folders: vec![],
            notes: vec![],
        };
        assert!(
            append(&mut plan, &mut HashSet::new(), &show, |_| Ok(
                BTreeMap::new()
            ))
            .is_err()
        );
        assert!(
            show.path
                .join("Season 01/release.S01E01.1080p.mkv")
                .exists()
        );
        fs::write(
            show.path.join("Season 01/duplicate.S01E01.mkv"),
            "duplicate",
        )
        .unwrap();
        append(&mut plan, &mut HashSet::new(), &show, |_| {
            Ok(BTreeMap::from([(1, "Pilot".into()), (3, "Third".into())]))
        })
        .unwrap();
        assert!(validate_plan(&plan, false).is_err());
        assert!(plan.moves.iter().all(|m| m.from.exists() && !m.to.exists()));
    }

    #[test]
    fn database_failure_rolls_back_series_files_and_folder_identity() {
        let (_temp, mut db, show) = fixture();
        let plan = plan(&db, &show);
        db.connection.execute_batch("CREATE TRIGGER reject_show BEFORE UPDATE OF path ON videos BEGIN SELECT RAISE(ABORT,'test failure'); END;").unwrap();
        assert!(apply(&mut db, &plan).is_err());
        assert!(plan.moves.iter().all(|m| m.from.exists() && !m.to.exists()));
        assert_eq!(db.videos().unwrap()[0].path, show.path);
        assert!(!plan.folders[0].1.exists());
    }

    #[test]
    fn existing_titles_and_unrecognized_multi_episode_names_are_preserved() {
        let path = Path::new("Show - S01E01 - Pilot.mkv");
        let (season, number, end) = episode(path).unwrap();
        assert_eq!(
            local_title(path, season, number, end).unwrap().as_deref(),
            Some("Pilot")
        );
        assert_eq!(
            episode(Path::new("Show.1x02.mkv")).map(|(s, e, _)| (s, e)),
            Some((1, 2))
        );
        assert!(episode(Path::new("Show.S01E01E02.mkv")).is_none());
        assert!(episode(Path::new("Show.S01E01-E03.mkv")).is_none());
        assert!(episode(Path::new("unrecognized.mkv")).is_none());
    }

    #[test]
    fn correctly_named_show_still_normalizes_seasons_and_undo_restores_them() {
        let (_temp, mut db, mut show) = fixture();
        let target = show.path.with_file_name("A Show (2020)");
        fs::rename(&show.path, &target).unwrap();
        let sources = db.sources().unwrap();
        db.apply_scan(
            &sources,
            &crate::index(&sources, &std::sync::atomic::AtomicBool::new(false), |_| {}),
        )
        .unwrap();
        db.save_match(Category::Series, &target, show.tmdb.as_ref().unwrap())
            .unwrap();
        show = db.videos().unwrap().remove(0);
        let plan = plan(&db, &show);
        let id = apply(&mut db, &plan).unwrap();
        assert!(!target.join("Season 01").exists());
        assert!(target.join("Season 1/A Show - S01E01 - Pilot.mkv").exists());
        undo(&mut db, id, &plan).unwrap();
        assert!(!target.join("Season 1").exists());
        assert!(target.join("Season 01/release.S01E01.1080p.mkv").exists());
    }

    #[test]
    fn undo_refuses_modified_episodes_before_recreating_old_show_folders() {
        let (_temp, mut db, show) = fixture();
        let plan = plan(&db, &show);
        let id = apply(&mut db, &plan).unwrap();
        let changed = plan.moves.iter().find(|m| m.video).unwrap();
        fs::write(&changed.to, b"changed episode").unwrap();
        assert!(undo(&mut db, id, &plan).is_err());
        assert!(!show.path.exists());
        assert_eq!(fs::read(&changed.to).unwrap(), b"changed episode");
    }
}
