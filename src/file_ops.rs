//! Previewed file changes with no-overwrite moves and a durable undo journal.
use crate::{
    Category, Source, Video,
    storage::{DbResult, LibraryDb},
};
use rusqlite::{OptionalExtension, TransactionBehavior, params};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
    time::UNIX_EPOCH,
};
use walkdir::WalkDir;
mod series;

pub const QUARANTINE: &str = ".mediashelf-cleanup";

/// All episode numbers contained in a recognized single or multi-episode filename.
pub fn episode_numbers(path: &Path) -> Option<(u32, Vec<u32>)> {
    series::numbers(path)
}

/// Local episode numbering and title, shared by the browser and organizer.
pub fn episode_details(
    path: &Path,
    siblings: &[PathBuf],
) -> DbResult<Option<(u32, u32, Option<String>)>> {
    series::details(path, siblings)
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FileMove {
    pub from: PathBuf,
    pub to: PathBuf,
    pub bytes: u64,
    modified_ns: u128,
    pub video: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OperationPlan {
    pub kind: String,
    pub sources: Vec<Source>,
    pub moves: Vec<FileMove>,
    pub notes: Vec<String>,
    #[serde(default)]
    pub folders: Vec<(PathBuf, PathBuf)>,
}

pub fn suggested_name(video: &Video) -> String {
    let original = video
        .path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned();
    let Some(matched) = &video.tmdb else {
        if video.category == Category::Movies {
            if let Ok(Some(local)) = crate::nfo::read(video) {
                if let Some(year) = local.year {
                    return format!(
                        "{} ({year}).{}",
                        sanitize_title(&local.title),
                        video.path.extension().unwrap_or_default().to_string_lossy()
                    );
                }
            }
        }
        return original;
    };
    let title = sanitize_title(&matched.title);
    if video.is_folder {
        return original;
    }
    let stem = match video.category {
        Category::Movies => {
            if matched.year() == "Unknown year" {
                title
            } else {
                format!("{title} ({})", matched.year())
            }
        }
        Category::Series => {
            if let Ok(Some(name)) = series::name(video, |_| Ok(Default::default())) {
                return name;
            }
            let re =
                regex::Regex::new(r"(?i)(s\d{1,3}e\d{1,3}(?:-?e\d{1,3})*|\b\d{1,3}x\d{1,3}\b)")
                    .unwrap();
            let Some(token) = re.find(&original) else {
                return original;
            };
            let token = token.as_str().to_uppercase();
            let token = if let Some((s, e)) = token.split_once('X') {
                format!(
                    "S{:02}E{:02}",
                    s.parse::<u32>().unwrap(),
                    e.parse::<u32>().unwrap()
                )
            } else {
                token
            };
            format!("{title} - {token}")
        }
    };
    format!(
        "{stem}.{}",
        video.path.extension().unwrap_or_default().to_string_lossy()
    )
}

fn episode_titles(
    video: &Video,
    season: u32,
) -> Result<std::collections::BTreeMap<u32, String>, String> {
    let saved = crate::credentials::load(&crate::credentials::token_path())
        .map_err(|_| "Could not load saved TMDB token")?;
    let token = if saved.is_empty() {
        std::env::var("TMDB_READ_ACCESS_TOKEN").unwrap_or_default()
    } else {
        saved
    };
    crate::metadata::episode_titles(
        video
            .tmdb
            .as_ref()
            .ok_or("Match this show before renaming")?
            .id,
        season,
        &token,
        "en-US",
    )
}

fn sanitize_title(title: &str) -> String {
    let result: String = title
        .chars()
        .map(|c| {
            if c.is_control() || "<>:\"/\\|?*".contains(c) {
                '_'
            } else {
                c
            }
        })
        .collect();
    let result = result.trim().trim_end_matches(['.', ' ']);
    if result.is_empty() {
        "Untitled".into()
    } else {
        result.to_string()
    }
}

pub(crate) fn standard_movie_folder(video: &Video) -> bool {
    video.category == Category::Movies
        && video.tmdb.as_ref().is_some_and(|m| {
            video
                .path
                .parent()
                .and_then(|p| p.file_name())
                .is_some_and(|n| {
                    n.to_string_lossy() == format!("{} - {}", m.year(), sanitize_title(&m.title))
                })
        })
}

pub fn rename_preview(
    sources: &[Source],
    requested: &[(Video, String)],
) -> DbResult<OperationPlan> {
    rename_preview_inner(sources, requested, false)
}

/// Apply the default movie and series organization, preserving matching sidecars.
pub fn organize_preview(
    sources: &[Source],
    requested: &[(Video, String)],
) -> DbResult<OperationPlan> {
    rename_preview_inner(sources, requested, true)
}

fn rename_preview_inner(
    sources: &[Source],
    requested: &[(Video, String)],
    folders: bool,
) -> DbResult<OperationPlan> {
    let mut plan = OperationPlan {
        folders: vec![],
        kind: if folders { "organize" } else { "rename" }.into(),
        sources: sources.to_vec(),
        moves: vec![],
        notes: vec![],
    };
    let mut seen = HashSet::new();
    for (video, name) in requested {
        if video.is_folder {
            if !folders {
                return Err("Use organization to rename a show folder.".into());
            }
            series::append(&mut plan, &mut seen, video, |season| {
                episode_titles(video, season)
            })?;
            continue;
        }
        let resolved = if folders && video.category == Category::Series {
            series::name(video, |season| episode_titles(video, season))?
        } else {
            None
        };
        let name = resolved.as_deref().unwrap_or(name);
        validate_name(name)?;
        if Path::new(name).extension() != video.path.extension() {
            return Err("Keep the original video extension unchanged.".into());
        }
        let target = if folders && video.category == Category::Movies {
            let local = if video.tmdb.is_none() {
                crate::nfo::read(video).ok().flatten()
            } else {
                None
            };
            let title = video
                .tmdb
                .as_ref()
                .map(|m| m.title.clone())
                .or_else(|| local.as_ref().map(|m| m.title.clone()))
                .ok_or("No saved match or readable movie NFO. Search & scrape this movie first.")?;
            let year = video
                .tmdb
                .as_ref()
                .map(|m| m.year().to_string())
                .or_else(|| local.as_ref().and_then(|m| m.year).map(|y| y.to_string()))
                .unwrap_or_default();
            if year.is_empty() || year == "Unknown year" {
                return Err(
                    "Movie year is missing. Choose a match with a release year before renaming."
                        .into(),
                );
            }
            let folder = format!("{year} - {}", sanitize_title(&title));
            validate_name(&folder)?;
            let root = sources
                .iter()
                .filter(|s| s.category == Category::Movies && video.path.starts_with(&s.path))
                .max_by_key(|s| s.path.components().count())
                .ok_or("Movie is outside its sources")?;
            let parent = video.path.parent().ok_or("Missing movie folder")?;
            let base = if parent == root.path {
                parent
            } else {
                parent.parent().ok_or("Missing movie folder parent")?
            };
            base.join(folder).join(name)
        } else {
            video.path.with_file_name(name)
        };
        if target == video.path || same_existing_path(&video.path, &target) {
            plan.notes
                .push(format!("Already named correctly: {}", video.path.display()));
        } else {
            add_move(&mut plan, &mut seen, &video.path, &target, true)?;
        }
        let standard = folders && video.category == Category::Movies;
        let parent = video.path.parent().ok_or("Missing movie folder")?;
        let single_movie = if standard {
            fs::read_dir(parent)?
                .filter_map(Result::ok)
                .filter(|e| crate::is_video(&e.path()))
                .count()
                == 1
        } else {
            false
        };
        let old_stem = video
            .path
            .file_stem()
            .and_then(|s| s.to_str())
            .ok_or("Invalid filename")?;
        let new_stem = target
            .file_stem()
            .and_then(|s| s.to_str())
            .ok_or("Invalid filename")?;
        let mut sidecars = fs::read_dir(video.path.parent().ok_or("Missing parent folder")?)?
            .collect::<Result<Vec<_>, _>>()?;
        // Standard names win deterministically when both generic and release-named
        // sidecars would move into a newly created movie folder.
        sidecars.sort_by_cached_key(|entry| {
            let name = entry.file_name().to_string_lossy().to_lowercase();
            let generic =
                name == "movie.nfo" || ["poster.", "fanart."].iter().any(|p| name.starts_with(p));
            (!generic, name)
        });
        for entry in sidecars {
            if !entry.file_type()?.is_file() {
                continue;
            }
            let path = entry.path();
            let ext = path
                .extension()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_ascii_lowercase();
            if !matches!(
                ext.as_str(),
                "srt"
                    | "ass"
                    | "ssa"
                    | "sub"
                    | "idx"
                    | "vtt"
                    | "nfo"
                    | "jpg"
                    | "jpeg"
                    | "png"
                    | "webp"
            ) {
                continue;
            }
            let filename = entry.file_name().to_string_lossy().into_owned();
            let lower = filename.to_ascii_lowercase();
            if standard
                && single_movie
                && (lower == "movie.nfo"
                    || ["poster.", "fanart."].iter().any(|p| lower.starts_with(p)))
            {
                let destination = target.with_file_name(&lower);
                if path != destination {
                    add_sidecar_move(&mut plan, &mut seen, &path, &destination)?;
                }
                continue;
            }
            let Some(suffix) = filename.strip_prefix(old_stem).filter(|s| {
                s.starts_with('.')
                    || ["-poster.", "-fanart.", "-thumb."]
                        .iter()
                        .any(|prefix| s.to_ascii_lowercase().starts_with(prefix))
            }) else {
                continue;
            };
            let output = if standard && suffix.eq_ignore_ascii_case(".nfo") {
                "movie.nfo".to_string()
            } else if standard && suffix.to_ascii_lowercase().starts_with("-poster.") {
                format!("poster.{ext}")
            } else if standard && suffix.to_ascii_lowercase().starts_with("-fanart.") {
                format!("fanart.{ext}")
            } else {
                format!("{new_stem}{suffix}")
            };
            let destination = target.with_file_name(output);
            if path != destination {
                if standard && matches!(ext.as_str(), "nfo" | "jpg" | "jpeg" | "png" | "webp") {
                    add_sidecar_move(&mut plan, &mut seen, &path, &destination)?;
                } else {
                    add_move(&mut plan, &mut seen, &path, &destination, false)?;
                }
            }
        }
    }
    validate_plan(&plan, false)?;
    Ok(plan)
}

pub fn cleanup_preview(sources: &[Source], category: Category) -> DbResult<OperationPlan> {
    let mut plan = OperationPlan {
        folders: vec![],
        kind: "cleanup".into(),
        sources: sources.to_vec(),
        moves: vec![],
        notes: vec![],
    };
    let mut seen = HashSet::new();
    let batch = format!(
        "{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(UNIX_EPOCH)?
            .as_nanos()
    );
    for source in sources.iter().filter(|s| s.category == category) {
        if !source.path.is_dir() {
            plan.notes
                .push(format!("Folder unavailable: {}", source.path.display()));
            continue;
        }
        let root = source.path.canonicalize()?;
        for entry in WalkDir::new(&root)
            .follow_links(false)
            .follow_root_links(false)
            .into_iter()
            .filter_entry(|e| {
                !e.file_name()
                    .to_string_lossy()
                    .eq_ignore_ascii_case(QUARANTINE)
            })
        {
            let entry = match entry {
                Ok(e) => e,
                Err(e) => {
                    plan.notes.push(e.to_string());
                    continue;
                }
            };
            if !entry.file_type().is_file() || !cleanup_candidate(entry.path()) {
                continue;
            }
            let target = root
                .join(QUARANTINE)
                .join(&batch)
                .join(entry.path().strip_prefix(&root)?);
            add_move(&mut plan, &mut seen, entry.path(), &target, false)?;
        }
    }
    validate_plan(&plan, false)?;
    Ok(plan)
}

pub fn cleanup_candidate(path: &Path) -> bool {
    let name = path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_ascii_lowercase();
    matches!(name.as_str(), "thumbs.db" | "desktop.ini" | ".ds_store")
        || matches!(
            path.extension()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_ascii_lowercase()
                .as_str(),
            "sfv" | "nzb"
        )
}

fn validate_name(name: &str) -> DbResult<()> {
    if name.is_empty()
        || name.trim() != name
        || name.ends_with('.')
        || name.chars().count() > 220
        || name
            .chars()
            .any(|c| c.is_control() || "<>:\"/\\|?*".contains(c))
    {
        return Err("Enter a filename without path separators, reserved characters, or trailing dots/spaces (maximum 220 characters).".into());
    }
    let stem = name
        .split('.')
        .next()
        .unwrap_or("")
        .trim()
        .to_ascii_uppercase();
    if matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || (stem.len() == 4
            && (stem.starts_with("COM") || stem.starts_with("LPT"))
            && matches!(stem.as_bytes()[3], b'1'..=b'9'))
    {
        return Err("That filename is reserved by Windows.".into());
    }
    Ok(())
}

fn fingerprint(path: &Path) -> DbResult<(u64, u128)> {
    let meta = fs::symlink_metadata(path)?;
    if !meta.file_type().is_file() {
        return Err(format!("Not a regular file: {}", path.display()).into());
    }
    Ok((
        meta.len(),
        meta.modified()?.duration_since(UNIX_EPOCH)?.as_nanos(),
    ))
}

fn add_move(
    plan: &mut OperationPlan,
    seen: &mut HashSet<PathBuf>,
    from: &Path,
    to: &Path,
    video: bool,
) -> DbResult<()> {
    if !seen.insert(from.to_path_buf()) {
        return Ok(());
    }
    let (bytes, modified_ns) = fingerprint(from)?;
    plan.moves.push(FileMove {
        from: from.to_path_buf(),
        to: to.to_path_buf(),
        bytes,
        modified_ns,
        video,
    });
    Ok(())
}

fn same_existing_path(from: &Path, to: &Path) -> bool {
    match (from.canonicalize(), to.canonicalize()) {
        (Ok(a), Ok(b)) => a == b,
        _ => false,
    }
}

fn add_sidecar_move(
    plan: &mut OperationPlan,
    seen: &mut HashSet<PathBuf>,
    from: &Path,
    to: &Path,
) -> DbResult<()> {
    if from == to || same_existing_path(from, to) {
        return Ok(());
    }
    if fs::symlink_metadata(to).is_ok() {
        // Existing standard artwork/NFO wins. Keep both files; never overwrite or delete.
        plan.notes.push(format!(
            "Kept existing sidecar {}; original retained at {}",
            to.display(),
            from.display()
        ));
        return Ok(());
    }
    if let Some(existing) = plan
        .moves
        .iter()
        .find(|m| m.to.to_string_lossy().to_lowercase() == to.to_string_lossy().to_lowercase())
    {
        plan.notes.push(format!(
            "Kept planned sidecar {} -> {}; alternative retained at {}",
            existing.from.display(),
            to.display(),
            from.display()
        ));
        return Ok(());
    }
    add_move(plan, seen, from, to, false)
}

fn validate_plan(plan: &OperationPlan, create: bool) -> DbResult<()> {
    if !matches!(plan.kind.as_str(), "rename" | "organize" | "cleanup") {
        return Err("Unknown file operation".into());
    }
    let mut destinations = HashSet::new();
    let mut folders = HashSet::new();
    for (from, to) in &plan.folders {
        let source = plan
            .sources
            .iter()
            .find(|s| s.category == Category::Series && from.parent() == Some(s.path.as_path()))
            .ok_or("Show folder is outside the series source")?;
        if plan.kind != "organize"
            || to.parent() != Some(source.path.as_path())
            || from.canonicalize()? != *from
            || (from != to && fs::symlink_metadata(to).is_ok())
            || !folders.insert(to.to_string_lossy().to_lowercase())
        {
            return Err("Show destination changed or conflicts with another show.".into());
        }
        validate_name(
            to.file_name()
                .and_then(|s| s.to_str())
                .ok_or("Invalid show folder name")?,
        )?;
    }
    for item in &plan.moves {
        let root = plan
            .sources
            .iter()
            .filter(|s| item.from.starts_with(&s.path))
            .map(|s| &s.path)
            .min_by_key(|p| p.components().count())
            .ok_or("File is outside the library folders")?;
        if root.canonicalize()? != *root || item.from.canonicalize()? != item.from {
            return Err("A folder changed since the preview. Generate a new preview.".into());
        }
        if fingerprint(&item.from)? != (item.bytes, item.modified_ns) {
            return Err(format!("File changed since preview: {}", item.from.display()).into());
        }
        if !destinations.insert(item.to.to_string_lossy().to_lowercase()) {
            return Err(
                format!("Two files would have the same name: {}", item.to.display()).into(),
            );
        }
        if fs::symlink_metadata(&item.to).is_ok() {
            return Err(format!("Destination already exists: {}", item.to.display()).into());
        }
        if plan.kind != "cleanup" {
            if plan.kind == "rename" && item.from.parent() != item.to.parent() {
                return Err("Rename must stay in the same folder".into());
            }
            if !item.to.starts_with(root) {
                return Err("Rename destination is outside the library folder".into());
            }
            validate_name(
                item.to
                    .file_name()
                    .and_then(|s| s.to_str())
                    .ok_or("Invalid filename")?,
            )?;
            let parent = item.to.parent().ok_or("Missing destination folder")?;
            let mut cursor = root.to_path_buf();
            for part in parent.strip_prefix(root)?.components() {
                if !matches!(part, std::path::Component::Normal(_)) {
                    return Err("Invalid destination folder".into());
                }
                cursor.push(part);
                match fs::symlink_metadata(&cursor) {
                    Ok(meta) if meta.is_dir() && cursor.canonicalize()? == cursor => {}
                    Ok(_) => {
                        return Err("Destination contains a redirected or invalid folder".into());
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                        if create {
                            fs::create_dir(&cursor)?;
                        }
                    }
                    Err(e) => return Err(e.into()),
                }
            }
        } else {
            if item.video
                || !cleanup_candidate(&item.from)
                || !item.to.starts_with(root.join(QUARANTINE))
            {
                return Err("Cleanup can only quarantine supported leftover files".into());
            }
            let parent = item.to.parent().ok_or("Missing quarantine folder")?;
            // Inspect each existing directory before creating children; reject redirects.
            let mut cursor = root.to_path_buf();
            for part in parent.strip_prefix(root)?.components() {
                cursor.push(part);
                match fs::symlink_metadata(&cursor) {
                    Ok(meta) if meta.is_dir() && cursor.canonicalize()? == cursor => {}
                    Ok(_) => {
                        return Err(
                            "Quarantine path contains a redirected or invalid folder".into()
                        );
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                        if create {
                            fs::create_dir(&cursor)?;
                        }
                    }
                    Err(e) => return Err(e.into()),
                }
            }
        }
    }
    Ok(())
}

#[cfg(windows)]
fn move_no_replace(from: &Path, to: &Path) -> std::io::Result<()> {
    use std::os::windows::ffi::OsStrExt;
    let from: Vec<u16> = from.as_os_str().encode_wide().chain(Some(0)).collect();
    let to: Vec<u16> = to.as_os_str().encode_wide().chain(Some(0)).collect();
    // Flags 0: no replacement and no copy/delete fallback across volumes.
    if unsafe {
        windows_sys::Win32::Storage::FileSystem::MoveFileExW(from.as_ptr(), to.as_ptr(), 0)
    } == 0
    {
        Err(std::io::Error::last_os_error())
    } else {
        Ok(())
    }
}

#[cfg(target_os = "macos")]
fn move_no_replace(from: &Path, to: &Path) -> std::io::Result<()> {
    use std::{ffi::CString, os::unix::ffi::OsStrExt};
    unsafe extern "C" {
        fn renamex_np(
            from: *const std::ffi::c_char,
            to: *const std::ffi::c_char,
            flags: u32,
        ) -> std::ffi::c_int;
    }
    let from = CString::new(from.as_os_str().as_bytes())?;
    let to = CString::new(to.as_os_str().as_bytes())?;
    // RENAME_EXCL preserves no-overwrite moves on SMB volumes without hard-link support.
    if unsafe { renamex_np(from.as_ptr(), to.as_ptr(), 0x00000004) } == 0 {
        Ok(())
    } else {
        Err(std::io::Error::last_os_error())
    }
}

#[cfg(not(any(windows, target_os = "macos")))]
fn move_no_replace(from: &Path, to: &Path) -> std::io::Result<()> {
    // Both destinations are on the same volume; link creation cannot overwrite.
    fs::hard_link(from, to)?;
    if let Err(e) = fs::remove_file(from) {
        let _ = fs::remove_file(to);
        return Err(e);
    }
    Ok(())
}

fn update_library(
    tx: &rusqlite::Transaction<'_>,
    plan: &OperationPlan,
    undo: bool,
    sources: &[Source],
) -> DbResult<()> {
    tx.execute_batch("PRAGMA defer_foreign_keys=ON;")?;
    for (from, to, bytes) in plan
        .moves
        .iter()
        .filter(|m| m.video)
        .map(|m| (&m.from, &m.to, m.bytes))
        .chain(
            plan.folders
                .iter()
                .filter(|(from, to)| from != to)
                .map(|(from, to)| (from, to, 0)),
        )
    {
        let (old, new) = if undo { (to, from) } else { (from, to) };
        for category in [Category::Movies, Category::Series] {
            let Some(source) = sources
                .iter()
                .find(|s| s.category == category && new.starts_with(&s.path))
            else {
                continue;
            };
            let relative = new.strip_prefix(&source.path)?;
            tx.execute(
                "UPDATE videos SET path=?1,relative_path=?2,bytes=?5 WHERE category=?3 AND path=?4",
                params![
                    text(new)?,
                    text(relative)?,
                    category.label(),
                    text(old)?,
                    i64::try_from(bytes)?
                ],
            )?;
            tx.execute(
                "UPDATE tmdb_matches SET path=?1 WHERE category=?2 AND path=?3",
                params![text(new)?, category.label(), text(old)?],
            )?;
            tx.execute(
                "UPDATE scraped_metadata SET path=?1 WHERE category=?2 AND path=?3",
                params![text(new)?, category.label(), text(old)?],
            )?;
        }
    }
    Ok(())
}

fn text(path: &Path) -> DbResult<&str> {
    path.to_str()
        .ok_or_else(|| "Filename cannot be stored as UTF-8".into())
}

pub fn apply(db: &mut LibraryDb, plan: &OperationPlan) -> DbResult<i64> {
    if plan.moves.is_empty() {
        return Err("Nothing selected to apply".into());
    }
    if db.sources()? != plan.sources {
        return Err("Library folders changed; generate a new preview".into());
    }
    if pending(db)? {
        return Err(
            "An interrupted operation needs Undo before starting another file operation.".into(),
        );
    }
    validate_plan(plan, false)?;
    db.connection.execute(
        "INSERT INTO file_operations(plan_json,status) VALUES (?1,'pending')",
        params![serde_json::to_string(plan)?],
    )?;
    let id = db.connection.last_insert_rowid();
    let result = (|| -> DbResult<()> {
        validate_plan(plan, true)?;
        let tx = db
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let mut moved = Vec::new();
        for item in &plan.moves {
            if fingerprint(&item.from).ok() != Some((item.bytes, item.modified_ns))
                || item.from.canonicalize().ok().as_ref() != Some(&item.from)
            {
                let rollback = rollback_moves(&moved, false);
                return Err(
                    format!("A source changed while applying the operation. {rollback}").into(),
                );
            }
            if let Err(e) = move_no_replace(&item.from, &item.to) {
                let rollback = rollback_moves(&moved, false);
                return Err(
                    format!("Could not move {}: {e}. {rollback}", item.from.display()).into(),
                );
            }
            moved.push(item.clone());
        }
        let saved = (|| -> DbResult<()> {
            update_library(&tx, plan, false, &plan.sources)?;
            tx.execute(
                "UPDATE file_operations SET status='applied' WHERE id=?1",
                [id],
            )?;
            Ok(())
        })();
        if let Err(e) = saved {
            drop(tx);
            let rollback = rollback_moves(&moved, false);
            return Err(format!("Database update failed: {e}. {rollback}").into());
        }
        if let Err(e) = tx.commit() {
            let rollback = rollback_moves(&moved, false);
            return Err(format!("Database commit failed: {e}. {rollback}").into());
        }
        Ok(())
    })();
    if let Err(e) = result {
        let restored = plan.moves.iter().all(|m| m.from.exists() && !m.to.exists());
        if restored {
            db.connection.execute(
                "UPDATE file_operations SET status='failed' WHERE id=?1",
                [id],
            )?;
            remove_empty_show_folders(plan, true);
        }
        return Err(format!("{e} Operation #{id}; use Undo if recovery is needed.").into());
    }
    remove_empty_show_folders(plan, false);
    Ok(id)
}

fn remove_empty_show_folders(plan: &OperationPlan, undo: bool) {
    if plan.kind == "organize" {
        for item in &plan.moves {
            let vacated = if undo { &item.to } else { &item.from };
            if !plan
                .folders
                .iter()
                .any(|(from, to)| vacated.starts_with(if undo { to } else { from }))
            {
                continue;
            }
            if let Some(source) = plan
                .sources
                .iter()
                .find(|s| s.category == Category::Series && vacated.starts_with(&s.path))
            {
                let mut parent = vacated.parent();
                while let Some(folder) = parent.filter(|p| *p != source.path) {
                    if fs::remove_dir(folder).is_err() {
                        break;
                    }
                    parent = folder.parent();
                }
            }
        }
    }
    for (from, to) in &plan.folders {
        let folder = if undo { to } else { from };
        for entry in WalkDir::new(folder)
            .follow_links(false)
            .follow_root_links(false)
            .contents_first(true)
            .into_iter()
            .filter_map(Result::ok)
        {
            if entry.file_type().is_dir() {
                let _ = fs::remove_dir(entry.path());
            }
        }
    }
}

fn rollback_moves(moved: &[FileMove], undo: bool) -> String {
    let mut failures = Vec::new();
    for item in moved.iter().rev() {
        let (from, to) = if undo {
            (&item.from, &item.to)
        } else {
            (&item.to, &item.from)
        };
        if let Err(e) = move_no_replace(from, to) {
            failures.push(format!("{}: {e}", from.display()));
        }
    }
    if failures.is_empty() {
        "Completed moves were rolled back".into()
    } else {
        format!("Recovery required: {}", failures.join("; "))
    }
}

fn pending(db: &LibraryDb) -> DbResult<bool> {
    Ok(db.connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM file_operations WHERE status='pending')",
        [],
        |r| r.get(0),
    )?)
}

pub fn last_undo(db: &LibraryDb) -> DbResult<Option<(i64, OperationPlan)>> {
    let row:Option<(i64,String)>=db.connection.query_row("SELECT id,plan_json FROM file_operations WHERE status IN ('applied','pending') ORDER BY CASE status WHEN 'pending' THEN 0 ELSE 1 END,id DESC LIMIT 1",[],|r|Ok((r.get(0)?,r.get(1)?))).optional()?;
    row.map(|(id, json)| Ok((id, serde_json::from_str(&json)?)))
        .transpose()
}

pub fn undo(db: &mut LibraryDb, id: i64, plan: &OperationPlan) -> DbResult<()> {
    let current = last_undo(db)?.ok_or("No file operation to undo")?;
    if current.0 != id || serde_json::to_string(&current.1)? != serde_json::to_string(plan)? {
        return Err("Operation history changed. Open a new Undo preview.".into());
    }
    let mut actual = Vec::new();
    for item in &plan.moves {
        if !db.sources()?.iter().any(|s| item.from.starts_with(&s.path)) {
            return Err("Restore the original library folder selection before undoing.".into());
        }
        if item.to.exists() && !item.from.exists() {
            if fingerprint(&item.to)? != (item.bytes, item.modified_ns)
                || item.to.canonicalize()? != item.to
            {
                return Err(
                    "A file or folder changed since the operation; undo was stopped.".into(),
                );
            }
            actual.push(item.clone());
        } else if item.from.exists()
            && !item.to.exists()
            && fingerprint(&item.from)? == (item.bytes, item.modified_ns)
        {
            continue;
        } else {
            return Err(format!(
                "Cannot restore without a collision or missing file: {}",
                item.from.display()
            )
            .into());
        }
    }
    let sources = db.sources()?;
    let restore = OperationPlan {
        kind: "organize".into(),
        sources: sources.clone(),
        folders: vec![],
        notes: vec![],
        moves: actual
            .iter()
            .map(|m| FileMove {
                from: m.to.clone(),
                to: m.from.clone(),
                ..m.clone()
            })
            .collect(),
    };
    validate_plan(&restore, false)?;
    validate_plan(&restore, true)?;
    let tx = db
        .connection
        .transaction_with_behavior(TransactionBehavior::Immediate)?;
    let mut moved = Vec::new();
    for item in &actual {
        if let Err(e) = move_no_replace(&item.to, &item.from) {
            let rollback = rollback_moves(&moved, true);
            return Err(format!("Undo failed: {e}. {rollback}").into());
        }
        moved.push(item.clone());
    }
    let saved = (|| -> DbResult<()> {
        update_library(&tx, plan, true, &sources)?;
        tx.execute(
            "UPDATE file_operations SET status='undone' WHERE id=?1",
            [id],
        )?;
        Ok(())
    })();
    if let Err(e) = saved {
        drop(tx);
        let rollback = rollback_moves(&moved, true);
        return Err(format!("Undo database update failed: {e}. {rollback}").into());
    }
    if let Err(e) = tx.commit() {
        let rollback = rollback_moves(&moved, true);
        return Err(format!("Undo commit failed: {e}. {rollback}").into());
    }
    remove_empty_show_folders(plan, true);
    Ok(())
}
