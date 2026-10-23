//! SQLite library storage. Media files themselves are never changed.
use crate::tmdb::TmdbMatch;
use crate::{Category, ScanReport, Source, Video};
use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};
use std::{
    error::Error,
    path::{Path, PathBuf},
    time::Duration,
};

pub type DbResult<T> = Result<T, Box<dyn Error + Send + Sync>>;

pub struct LibraryDb {
    pub(crate) connection: Connection,
}

pub fn database_path() -> PathBuf {
    #[cfg(target_os = "macos")]
    let root = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_default()
        .join("Library/Application Support");
    #[cfg(not(target_os = "macos"))]
    let root = std::env::var_os("LOCALAPPDATA")
        .or_else(|| std::env::var_os("XDG_CONFIG_HOME"))
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            std::env::var_os("HOME")
                .map(PathBuf::from)
                .unwrap_or_default()
                .join(".config")
        });
    root.join("MediaShelf").join("library.sqlite3")
}

#[test]
#[cfg(target_os = "macos")]
fn macos_library_uses_application_support() {
    let home = std::env::var_os("HOME").expect("macOS user home");
    assert_eq!(
        database_path(),
        PathBuf::from(home).join("Library/Application Support/MediaShelf/library.sqlite3")
    );
}

impl LibraryDb {
    pub fn open(path: &Path) -> DbResult<Self> {
        if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
            std::fs::create_dir_all(parent)?;
        }
        let mut connection = Connection::open(path)?;
        connection.busy_timeout(Duration::from_secs(5))?;
        connection.pragma_update(None, "foreign_keys", "ON")?;
        let version: i64 = connection.pragma_query_value(None, "user_version", |r| r.get(0))?;
        if version > 5 {
            return Err("This database was created by a newer Media Shelf version.".into());
        }
        let tx = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        tx.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS sources (
                id INTEGER PRIMARY KEY,
                category TEXT NOT NULL CHECK(category IN ('Movies', 'Series')),
                path TEXT NOT NULL,
                UNIQUE(category, path)
            );
            CREATE TABLE IF NOT EXISTS videos (
                id INTEGER PRIMARY KEY,
                category TEXT NOT NULL CHECK(category IN ('Movies', 'Series')),
                path TEXT NOT NULL,
                relative_path TEXT NOT NULL,
                bytes INTEGER NOT NULL CHECK(bytes >= 0),
                first_seen_at INTEGER NOT NULL DEFAULT (unixepoch()),
                last_seen_at INTEGER NOT NULL DEFAULT (unixepoch()),
                UNIQUE(category, path)
            );
            CREATE TABLE IF NOT EXISTS settings (key TEXT PRIMARY KEY, value TEXT NOT NULL);
            CREATE TABLE IF NOT EXISTS scans (
                id INTEGER PRIMARY KEY,
                completed_at INTEGER NOT NULL DEFAULT (unixepoch()),
                file_count INTEGER NOT NULL,
                warning_count INTEGER NOT NULL
            );
            CREATE TABLE IF NOT EXISTS tmdb_matches (
                category TEXT NOT NULL,
                path TEXT NOT NULL,
                tmdb_id INTEGER NOT NULL CHECK(tmdb_id > 0),
                title TEXT NOT NULL,
                original_title TEXT NOT NULL,
                release_date TEXT,
                overview TEXT NOT NULL,
                matched_at INTEGER NOT NULL DEFAULT (unixepoch()),
                PRIMARY KEY(category, path),
                FOREIGN KEY(category, path) REFERENCES videos(category, path) ON DELETE CASCADE
            );
            CREATE TABLE IF NOT EXISTS file_operations (
                id INTEGER PRIMARY KEY,
                plan_json TEXT NOT NULL,
                status TEXT NOT NULL,
                created_at INTEGER NOT NULL DEFAULT (unixepoch())
            );
            CREATE UNIQUE INDEX IF NOT EXISTS one_pending_file_operation ON file_operations(status) WHERE status='pending';
            CREATE TABLE IF NOT EXISTS scraped_metadata (
                category TEXT NOT NULL, path TEXT NOT NULL, data_json TEXT NOT NULL,
                scraped_at INTEGER NOT NULL DEFAULT (unixepoch()),
                PRIMARY KEY(category,path),
                FOREIGN KEY(category,path) REFERENCES videos(category,path) ON DELETE CASCADE
            );
        ",
        )?;
        let has_folder_column: bool = tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM pragma_table_info('videos') WHERE name='is_folder')",
            [],
            |r| r.get(0),
        )?;
        if !has_folder_column {
            tx.execute_batch("ALTER TABLE videos ADD COLUMN is_folder INTEGER NOT NULL DEFAULT 0 CHECK(is_folder IN (0,1));")?;
        }
        tx.pragma_update(None, "user_version", 5)?;
        tx.commit()?;
        Ok(Self { connection })
    }

    /// Import v0.1 settings once. Keep the old file as a backup; never resurrect
    /// removed sources on a later startup. Malformed JSON rolls the migration back.
    pub fn migrate_folders(&mut self, legacy_path: &Path) -> DbResult<()> {
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let migrated: Option<String> = tx
            .query_row(
                "SELECT value FROM settings WHERE key = 'folders_json_imported'",
                [],
                |r| r.get(0),
            )
            .optional()?;
        if migrated.is_none() {
            let count: i64 = tx.query_row("SELECT count(*) FROM sources", [], |r| r.get(0))?;
            if count == 0 {
                let sources: Vec<Source> = match std::fs::read(legacy_path) {
                    Ok(bytes) => serde_json::from_slice(&bytes)?,
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => vec![],
                    Err(e) => return Err(e.into()),
                };
                for source in sources {
                    tx.execute(
                        "INSERT OR IGNORE INTO sources(category, path) VALUES (?1, ?2)",
                        params![source.category.label(), path_text(&source.path)?],
                    )?;
                }
            }
            tx.execute(
                "INSERT INTO settings(key, value) VALUES ('folders_json_imported', '1')",
                [],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    pub fn sources(&self) -> DbResult<Vec<Source>> {
        read_sources(&self.connection)
    }
    pub fn show_missing_episodes(&self) -> DbResult<bool> {
        Ok(self
            .connection
            .query_row(
                "SELECT value FROM settings WHERE key='show_missing_episodes'",
                [],
                |r| r.get::<_, String>(0),
            )
            .optional()?
            .as_deref()
            == Some("true"))
    }

    pub fn set_show_missing_episodes(&mut self, enabled: bool) -> DbResult<()> {
        self.connection.execute("INSERT INTO settings(key,value) VALUES ('show_missing_episodes',?1) ON CONFLICT(key) DO UPDATE SET value=excluded.value", [if enabled {"true"} else {"false"}])?;
        Ok(())
    }

    pub fn videos(&self) -> DbResult<Vec<Video>> {
        let mut videos = read_videos(&self.connection)?;
        let folders: std::collections::HashSet<_> = videos
            .iter()
            .filter(|v| v.is_folder)
            .map(|v| v.path.clone())
            .collect();
        videos.retain(|v| {
            v.category != Category::Series
                || v.is_folder
                || !v.path.ancestors().skip(1).any(|p| folders.contains(p))
        });
        crate::artwork::attach_items(
            videos.iter_mut().filter(|v| v.category == Category::Movies),
            &self.sources()?,
        );
        Ok(videos)
    }

    pub fn import_local_nfos(&mut self) -> DbResult<Vec<String>> {
        if self.ensure_files_settled().is_err() {
            return Ok(vec![]);
        }
        let mut entries = vec![];
        let mut warnings = vec![];
        for video in read_videos(&self.connection)? {
            if video.category != Category::Movies
                || video.tmdb.is_some()
                || video.metadata.is_some()
            {
                continue;
            }
            match crate::nfo::read(&video) {
                Ok(Some(local)) => {
                    if let Some(data) = local.metadata {
                        entries.push((video.category, video.path, data));
                    }
                }
                Ok(None) => {}
                Err(e) => warnings.push(format!("{}: {e}", video.path.display())),
            }
        }
        if !entries.is_empty() {
            self.save_scraped(&entries)?;
        }
        Ok(warnings)
    }

    pub fn save_match(
        &mut self,
        category: Category,
        path: &Path,
        matched: &TmdbMatch,
    ) -> DbResult<()> {
        if category != matched.category {
            return Err("Choose a match from the same movie or series category.".into());
        }
        self.connection.execute("INSERT INTO tmdb_matches(category,path,tmdb_id,title,original_title,release_date,overview)
            VALUES (?1,?2,?3,?4,?5,?6,?7) ON CONFLICT(category,path) DO UPDATE SET
            tmdb_id=excluded.tmdb_id,title=excluded.title,original_title=excluded.original_title,
            release_date=excluded.release_date,overview=excluded.overview,matched_at=unixepoch()",
            params![category.label(),path_text(path)?,matched.id,matched.title,matched.original_title,matched.release_date,matched.overview])?;
        Ok(())
    }

    pub fn clear_match(&mut self, category: Category, path: &Path) -> DbResult<()> {
        let tx = self.connection.transaction()?;
        tx.execute(
            "DELETE FROM scraped_metadata WHERE category=?1 AND path=?2",
            params![category.label(), path_text(path)?],
        )?;
        tx.execute(
            "DELETE FROM tmdb_matches WHERE category=?1 AND path=?2",
            params![category.label(), path_text(path)?],
        )?;
        tx.commit()?;
        Ok(())
    }

    pub fn save_scraped(
        &mut self,
        entries: &[(Category, PathBuf, crate::metadata::Metadata)],
    ) -> DbResult<()> {
        self.ensure_files_settled()?;
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        for (category, path, data) in entries {
            if *category != data.identity.category {
                return Err("Scrape category does not match the library item".into());
            }
            let m = &data.identity;
            tx.execute("INSERT INTO tmdb_matches(category,path,tmdb_id,title,original_title,release_date,overview) VALUES (?1,?2,?3,?4,?5,?6,?7)
                ON CONFLICT(category,path) DO UPDATE SET tmdb_id=excluded.tmdb_id,title=excluded.title,original_title=excluded.original_title,release_date=excluded.release_date,overview=excluded.overview,matched_at=unixepoch()",
                params![category.label(),path_text(path)?,m.id,m.title,m.original_title,m.release_date,m.overview])?;
            tx.execute("INSERT INTO scraped_metadata(category,path,data_json) VALUES (?1,?2,?3) ON CONFLICT(category,path) DO UPDATE SET data_json=excluded.data_json,scraped_at=unixepoch()",
                params![category.label(),path_text(path)?,serde_json::to_string(data)?])?;
        }
        tx.commit()?;
        Ok(())
    }

    /// Source changes and removal of their cached entries are one transaction.
    pub fn set_sources(&mut self, sources: &[Source]) -> DbResult<()> {
        self.ensure_files_settled()?;
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        tx.execute("DELETE FROM sources", [])?;
        for source in sources {
            tx.execute(
                "INSERT INTO sources(category, path) VALUES (?1, ?2)",
                params![source.category.label(), path_text(&source.path)?],
            )?;
        }
        for video in read_videos(&tx)? {
            if !sources
                .iter()
                .any(|s| s.category == video.category && video.path.starts_with(&s.path))
            {
                tx.execute(
                    "DELETE FROM videos WHERE category=?1 AND path=?2",
                    params![video.category.label(), path_text(&video.path)?],
                )?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    /// Clean scans remove missing entries. Incomplete scans only merge discoveries,
    /// keeping offline or unreadable files cached. Cancelled scans write nothing.
    pub fn apply_scan(&mut self, sources: &[Source], report: &ScanReport) -> DbResult<()> {
        if report.cancelled {
            return Ok(());
        }
        self.ensure_files_settled()?;
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        if read_sources(&tx)? != sources {
            return Err(
                "Folder selections changed during this scan. Restart the app and rescan.".into(),
            );
        }
        let previous = read_videos(&tx)?;
        tx.execute_batch(
            "CREATE TEMP TABLE IF NOT EXISTS scan_seen (
            category TEXT NOT NULL, path TEXT NOT NULL, PRIMARY KEY(category, path));
            DELETE FROM scan_seen;",
        )?;
        {
            let mut insert = tx.prepare("INSERT INTO videos(category, path, relative_path, bytes, is_folder)
                VALUES (?1, ?2, ?3, ?4, ?5) ON CONFLICT(category, path) DO UPDATE SET
                relative_path=excluded.relative_path, bytes=excluded.bytes, is_folder=excluded.is_folder, last_seen_at=unixepoch()")?;
            let mut seen =
                tx.prepare("INSERT OR IGNORE INTO scan_seen(category, path) VALUES (?1, ?2)")?;
            for video in &report.videos {
                if (video.is_folder && video.category != Category::Series)
                    || !sources
                        .iter()
                        .any(|s| s.category == video.category && video.path.starts_with(&s.path))
                {
                    return Err("Scan returned a file outside the selected folders.".into());
                }
                insert.execute(params![
                    video.category.label(),
                    path_text(&video.path)?,
                    path_text(&video.relative_path)?,
                    i64::try_from(video.bytes)?,
                    video.is_folder
                ])?;
                seen.execute(params![video.category.label(), path_text(&video.path)?])?;
                if video.is_folder {
                    let cached: Vec<_> = previous
                        .iter()
                        .filter(|v| {
                            v.category == Category::Series
                                && !v.is_folder
                                && v.path.starts_with(&video.path)
                        })
                        .collect();
                    // Keep episode metadata locally; shallow indexing cannot confirm episode deletions.
                    for old in &cached {
                        seen.execute(params![old.category.label(), path_text(&old.path)?])?;
                    }
                    if let Some(old) = cached
                        .into_iter()
                        .max_by_key(|v| (v.metadata.is_some(), v.tmdb.is_some()))
                    {
                        tx.execute("INSERT OR IGNORE INTO tmdb_matches(category,path,tmdb_id,title,original_title,release_date,overview,matched_at)
                            SELECT category,?1,tmdb_id,title,original_title,release_date,overview,matched_at FROM tmdb_matches WHERE category='Series' AND path=?2",
                            params![path_text(&video.path)?, path_text(&old.path)?])?;
                        tx.execute("INSERT OR IGNORE INTO scraped_metadata(category,path,data_json,scraped_at)
                            SELECT category,?1,data_json,scraped_at FROM scraped_metadata WHERE category='Series' AND path=?2",
                            params![path_text(&video.path)?, path_text(&old.path)?])?;
                    }
                }
            }
        }
        if report.warnings.is_empty() {
            tx.execute(
                "DELETE FROM videos WHERE NOT EXISTS (SELECT 1 FROM scan_seen
                WHERE scan_seen.category=videos.category AND scan_seen.path=videos.path)",
                [],
            )?;
        }
        tx.execute(
            "INSERT INTO scans(file_count, warning_count) VALUES (?1, ?2)",
            params![
                i64::try_from(report.videos.len())?,
                i64::try_from(report.warnings.len())?
            ],
        )?;
        tx.commit()?;
        Ok(())
    }

    fn ensure_files_settled(&self) -> DbResult<()> {
        let pending: bool = self.connection.query_row(
            "SELECT EXISTS(SELECT 1 FROM file_operations WHERE status='pending')",
            [],
            |r| r.get(0),
        )?;
        if pending {
            return Err(
                "An interrupted file operation needs Undo before rescanning or changing folders."
                    .into(),
            );
        }
        Ok(())
    }
}

fn path_text(path: &Path) -> DbResult<&str> {
    path.to_str()
        .ok_or_else(|| "A path contains characters SQLite cannot store as UTF-8.".into())
}

fn category(row: &rusqlite::Row<'_>) -> rusqlite::Result<Category> {
    match row.get::<_, String>(0)?.as_str() {
        "Movies" => Ok(Category::Movies),
        "Series" => Ok(Category::Series),
        _ => Err(rusqlite::Error::InvalidQuery),
    }
}

fn read_sources(connection: &Connection) -> DbResult<Vec<Source>> {
    let mut stmt = connection.prepare("SELECT category, path FROM sources ORDER BY id")?;
    Ok(stmt
        .query_map([], |r| {
            Ok(Source {
                category: category(r)?,
                path: r.get::<_, String>(1)?.into(),
            })
        })?
        .collect::<Result<Vec<_>, _>>()?)
}

fn read_videos(connection: &Connection) -> DbResult<Vec<Video>> {
    let mut stmt = connection.prepare(
        "SELECT v.category, v.path, v.relative_path, v.bytes,
        m.tmdb_id,m.title,m.original_title,m.release_date,m.overview,s.data_json,v.is_folder,date(v.first_seen_at,'unixepoch','localtime') FROM videos v
        LEFT JOIN tmdb_matches m ON m.category=v.category AND m.path=v.path
        LEFT JOIN scraped_metadata s ON s.category=v.category AND s.path=v.path
        ORDER BY v.category, v.path COLLATE NOCASE",
    )?;
    Ok(stmt
        .query_map([], |r| {
            Ok(Video {
                added_date: r.get(11)?,
                is_folder: r.get(10)?,
                artwork: Default::default(),
                category: category(r)?,
                path: r.get::<_, String>(1)?.into(),
                relative_path: r.get::<_, String>(2)?.into(),
                bytes: r.get::<_, i64>(3)? as u64,
                metadata: r
                    .get::<_, Option<String>>(9)?
                    .map(|json| {
                        serde_json::from_str(&json).map_err(|e| {
                            rusqlite::Error::FromSqlConversionFailure(
                                9,
                                rusqlite::types::Type::Text,
                                Box::new(e),
                            )
                        })
                    })
                    .transpose()?,
                tmdb: r
                    .get::<_, Option<i64>>(4)?
                    .map(|id| -> rusqlite::Result<TmdbMatch> {
                        Ok(TmdbMatch {
                            category: category(r)?,
                            id,
                            title: r.get(5)?,
                            original_title: r.get(6)?,
                            release_date: r.get(7)?,
                            overview: r.get(8)?,
                        })
                    })
                    .transpose()?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?)
}
