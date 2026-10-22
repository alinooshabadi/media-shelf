# User guide

A Rust media manager for Windows and macOS with a minimal light interface, a library/detail layout and
a separate search, preview, field-selection, and scrape workflow.

## Run

Close any older running version, then double-click `MediaShelf-v0.6.8.exe`
next to this project folder (or `MediaShelf.exe` if it was replaced).
On macOS, open **Media Shelf.app** instead. In the app:

1. Choose **Movies** or **TV shows**.
2. Open **Data sources** and browse for a folder, or paste a folder path and click **Add**.
3. The scan starts automatically. Movies are recursive; TV sources index only immediate show folders. Add more folders if needed.
4. Filter by title/path. Select rows to view details; Ctrl-click (Command-click on Mac) selects multiple movies.
5. Right-click a movie, TV show or episode for **Search & scrape** and **Rename**. Right-clicking a selected item keeps the multi-selection. Rename applies the default format immediately; selected items need a saved match or local NFO with title/year.
6. Click **Update sources** after adding files to your media folders.

Folders and video records are saved to
`%LOCALAPPDATA%\MediaShelf\library.sqlite3` on Windows. SQLite is bundled into
the executable; no separate database installation or server is needed.

The saved library loads on startup without rescanning. Click **Update sources**
to refresh it. Upgrading from v0.1 imports your existing `folders.json` once,
leaves that file as a backup, and lets you rescan to populate the database.
Removing a source removes its cached files unless another source in that
category still covers them. It does not delete folders or media files.

Scanning runs in the background. Unavailable folders and permission errors appear
under scan warnings. A cancelled scan keeps the previous library. A scan with
warnings merges discoveries while retaining cached records, including entries
on offline drives. Missing files are removed from the database only after a
complete scan without warnings. Slow or disconnected network drives can delay
cancellation until the current filesystem operation returns.

The database stores categories, full and relative paths, file sizes, first/last
seen timestamps, and completed scan counts/timestamps. Updates use transactions.
To back up the library, close the app and copy `library.sqlite3`.
On macOS, the database is `~/Library/Application Support/MediaShelf/library.sqlite3`.
Artwork, token files and output backups also live in this directory.
On Linux the desktop uses `$XDG_CONFIG_HOME/MediaShelf/library.sqlite3`, or
`~/.config/MediaShelf/library.sqlite3` when that variable is unset.

Supported extensions (case-insensitive): MKV, MP4, M4V, AVI, MOV, WMV, WEBM,
MPG, MPEG, TS, M2TS, MTS, VOB, OGV, FLV, DIVX. Discovery is based on extensions;
it does not validate video contents. Symlinks and junctions are not traversed.
Overlapping folders are deduplicated within each category.

This version scans on request. Folder watching, moving/renaming whole
movie folders, and service installation are future work.

### Local movie titles

For unmatched movies, the library and TMDB search prefer a source-relative
`YYYY - Title` parent folder. For example, `1976 - Taxi Driver/video.mp4`
appears as Taxi Driver with year 1976. Otherwise the app extracts the title/year
from the video filename and removes common release/codec suffixes. Confirmed
TMDB matches keep priority. Local hints are not treated as confirmed matches.
Sample/samples folders, `.deletedByTMM`, `.mediashelf-cleanup` and filenames
ending in a separate `sample` token are excluded. Click Update sources with the
source online to prune previously indexed excluded clips. No media is deleted.
Existing movie NFOs are imported as described below.
### Existing artwork

Local artwork loads automatically at startup and when the library refreshes,
without a TMDB match or scrape. Supported extensions are JPG, JPEG, PNG and WebP,
with case-insensitive filenames. Poster names: `<video-stem>-poster`,
`<video-stem>.poster`, `poster`, `folder`. Fanart names: `<video-stem>-fanart`,
`<video-stem>.fanart`, `fanart`, `backdrop`. Filename-specific artwork takes
priority within a directory; local files take priority over downloaded artwork.
TV show artwork is loaded when selecting a show, so indexing does not enumerate show contents. Previously indexed episodes also look in the inferred show folder. The overview, Artwork tab and
No artwork filter recognize these local files. Artwork is read in place, without
creating a provider match or changing the files. Metadata still requires scraping;
movie NFO import is supported; other artwork categories are not implemented yet.
### Import existing movie NFOs

On startup and after Update sources, unmatched movies can import `movie.nfo` or
`<video-stem>.nfo` without TMDB access. A generic movie.nfo is used only in a
folder with one main video, avoiding accidental assignment across a flat library.
A valid TMDB ID imports supported metadata into SQLite. Existing app matches and
metadata retain priority. NFO files are read only; unsupported fields remain in
the original file. An NFO with only a title and year still enables Rename without
inventing a provider ID. TV NFO import is not supported yet. XML DTDs are rejected
and file sizes are capped. Scan warnings report invalid NFO files.
Paths in Overview, Media files and Data sources are clickable. Click a media
path to open its containing folder in Windows Explorer. Offline folders show an
error instead of launching an invalid location.

## Rename files

Right-click an item and choose **Rename**, or use the Rename button in Overview.
Rename applies immediately, without a filename editor or confirmation dialog.
Movies require a saved match or a readable local NFO with title/year and use:

```text
2001 - Fat Girl/
  Fat Girl (2001).mp4
  movie.nfo
  poster.jpg
  fanart.jpg
```

Existing related NFO and artwork files are standardized and moved with the video;
subtitles keep their language suffixes. Artwork retains its actual image extension
(e.g. PNG stays PNG). Missing NFO/artwork is not generated by Rename; use Search &
scrape to fetch it. Re-scraping an organized movie uses the same generic sidecar names.
Generic artwork is moved only when the original folder contains one video, to
avoid assigning shared artwork to the wrong movie. Conflicting destinations stop
the whole operation for video/subtitle collisions; no existing file is overwritten. Existing standard artwork/NFO files take priority: when poster, fanart or movie.nfo
already exists, it is kept and the alternative source file is left untouched.
If several sidecars would move to the same new destination, standard generic
names take priority, with a stable filename order; remaining alternatives are retained.
Original folders and unrelated
files are retained. Undo restores the original file paths, including after restart.

A movie directly in a source root gets a new subfolder. A movie already in a folder
gets the new folder beside its current folder, avoiding repeated nesting.
**Rename & cleanup → Rename all matched files…** applies the defaults to matched
files in the current category. Indexed series use `Show (Year)/Season N/Show - S01E01 - Episode title.ext`.
Only Rename scans the selected show's episodes. Matching local NFO titles and
existing standard filenames take priority; missing episode titles are fetched
from TMDB in English, once per season. Save your token in Settings first.
If a needed title cannot be loaded, no files move. Subtitles (including language
suffixes), NFOs and thumbnails follow the episode names; other files are retained.
Unrecognized and multi-episode filenames stay unchanged and are reported.
Empty old directories are removed after a successful series rename; Undo recreates
the original paths. Existing show folders are never merged.
SQLite paths and saved matches/metadata are updated after successful moves.

## Cleanup and Undo

Choose **Rename & cleanup → Preview folder cleanup…** to search all source folders
in the current category for `Thumbs.db`, `desktop.ini`, `.DS_Store`, `.sfv`, and
`.nzb` files. Select candidates and click **Quarantine selected files**. Candidates
start unchecked. Cleanup moves them into a unique batch under
`.mediashelf-cleanup` inside each source root. Media scans and later cleanup
previews exclude that directory. Cleanup does not remove videos, subtitles,
NFOs, artwork, arbitrary text files, or active download fragments.

**Rename & cleanup → Undo / recover last operation…** previews restoration of the
latest rename/cleanup operation, including after restarting. Click **Restore
original paths** to apply it. Undo refuses collisions and changed files. Empty
quarantine directories are left in place; there is no permanent-delete action.

An operation journal is committed to SQLite before file changes. On a runtime
failure the app attempts to roll back completed moves. After an interruption,
Undo can restore the moves that completed; rescanning and changing source folders
are blocked while an operation is pending. Keep the original source folders
available for recovery. Close other versions of the app before organizing files.
Previews check file size, modification time, paths, and target collisions;
they do not hash entire videos. Use **Update sources** after external file edits.

### Movie NFO format

Movie NFO exports follow the supplied tinyMediaManager 3.1 sample structure:
standalone UTF-8 XML, legacy and unique IDs, IMDb as default when available,
themoviedb ratings, plot/outline, mpaa/certification, actors with available
photo/profile links, producers, languages, Kodi YouTube trailer URLs, fileinfo
and application metadata sections. The creator comment identifies Media Shelf.
Unknown technical, playback, date-added, source/edition and collection-ID values
are empty; media probing and merging unsupported fields from existing XML are not implemented yet.
The original filename is retained from the first scrape when available.
Older saved cast entries remain readable; re-scrape Actors to fetch profile data.
Existing NFOs change only when scraping with Write NFO and replacement enabled;
replacement keeps a backup but does not merge unsupported fields from old XML.
## Search, preview and scrape

1. Open **Settings** and paste your **API Read Access Token** from
   [your TMDB account settings](https://www.themoviedb.org/settings/api).
   Use the Read Access Token, rather than the shorter API key.
2. Select movies or a TV show in the library, then click **Search & scrape**.
3. Search starts automatically when a token is available. Edit the title and
   language and click **Search** to refine it. TMDB is the available scraper.
4. Candidates appear on the left. Selecting one loads full details, poster,
   tagline, plot, and cast/crew on the right. Previewing does not save metadata.
5. Under **Scrape following items**, check the fields to apply. Unchecked fields
   retain their previous library values. **Fill missing fields only** preserves
   existing values even for checked fields.
6. Click **OK — Scrape** to save selected metadata, cache selected poster/fanart,
   and write selected artwork beside the media. Optional **Write NFO** exports
   movie metadata to `movie.nfo` in an organized movie folder (otherwise `<video>.nfo`), or show metadata to `<show folder>/tvshow.nfo`.
   **Cancel** before confirmation leaves the library and media outputs unchanged.

Existing NFO/artwork files are kept unless **Replace existing NFO/artwork** is
checked. Replacements are backed up under `%LOCALAPPDATA%/MediaShelf/output-backups`.
Output failures are reported separately; saved library metadata remains available.
Artwork is also cached under `%LOCALAPPDATA%/MediaShelf/artwork` for offline views.

Multiple selected movies are processed in a review queue, with **Skip** and
**Cancel**. The toolbar dropdown includes **Re-scrape using saved IDs** and
**Search & scrape unscraped items**. Re-scraping uses the same field-selection
confirmation; it does not silently update every field.

TV sources index only first-level show folders, without enumerating seasons or
episodes. Existing episode metadata stays in the local database and the library
shows one entry per show. A show scrape updates that entry and can write
`tvshow.nfo`. Rename scans just the selected show and resolves episode titles;
full episode plot scraping and missing-episode discovery are not implemented.
Put each show inside its own folder; flat episodes in the source root are not indexed.

The main detail pane contains **Overview**, **Cast & crew**, **Media files**, **Artwork**,
and **Trailer** tabs. Rows show New, Matched or Scraped status. Hover over the status for metadata, artwork and NFO details.

Click **Save token** in API settings once. The saved token loads automatically on startup.
On Windows it is protected with user-scoped DPAPI in
`%LOCALAPPDATA%\MediaShelf\tmdb-token.dat`; it is not stored in SQLite.
**Forget token** removes the saved copy and clears the current field.
On macOS and Linux, the user-local token file has owner-only permissions (0600).
It is not encrypted; macOS Keychain storage is not implemented.
You may alternatively set `TMDB_READ_ACCESS_TOKEN` before launching the app.
The editable search title, category, language, page, and selected provider IDs are sent to TMDB;
local folder paths and media files are not sent. Search runs in the background
with a timeout. It requires an internet connection and a valid token.

The app includes TMDB credits and the approved logo in **TMDB settings & credits**.
This product uses the TMDB API but is not endorsed or certified by TMDB.

## Build from source

With stable Rust and a working Windows C/C++ toolchain installed:

```powershell
cargo build --release
cargo test
```

Or run `build.ps1`, which also recognizes the portable toolchain prepared in this
workspace. On macOS, install stable Rust and Xcode command-line tools, then run:

```sh
./build-macos.sh
open "target/macos/Media Shelf.app"
```

This builds for the current Mac's architecture and creates a locally signed app
bundle. Public Developer ID signing, notarization and universal builds are not included.
The shared library in `src/lib.rs` contains no GUI dependencies.

## Raspberry Pi / headless use

On the Pi, install Rust and build without the desktop feature:

```sh
cargo build --release --no-default-features
./target/release/media-shelf --scan movies /mnt/media/Movies
./target/release/media-shelf --scan series /mnt/media/Series
```

The command scans movies recursively or lists first-level series folders, and writes JSON containing paths,
categories, sizes, and warnings. Exit status is 0 for a complete scan, 2 when the
scan has warnings, and 1 for invalid command-line arguments. It is not yet a
background monitoring service. This one-off JSON command does not update the
desktop database; the shared `storage::LibraryDb` API is ready for a future
service. A Pi binary has not been built or tested here.

## Layout

- `src/lib.rs`: reusable read-only scanner and filesystem tests.
- `src/desktop.rs`: Windows-capable egui interface and saved folder settings.
- `src/storage.rs`: SQLite schema, JSON migration, and transactional library updates.
- `src/tmdb.rs`: TMDB HTTP client, response models, and local HTTP mock tests.
- `src/desktop/matching.rs`: token settings, candidate previews and scrape queue.
- `src/desktop/library_view.rs`: library table, TV show tree, and detail tabs.
- `src/metadata.rs`: details API, field merging, artwork cache, NFO and output backups.
- `tests/scrape_flow.rs`: selected-field, output, persistence, grouping and rename regression tests.
- `src/file_ops.rs`: filename suggestions, previews, quarantine, journaling, rollback, and undo.
- `src/desktop/file_tools.rs`: rename, cleanup, and restore dialogs.
- `tests/file_operations.rs`: real filesystem and database operation tests.
- `tests/persistence.rs`: file-backed database integration tests.
- `src/main.rs`: desktop/headless entry point.
- `Cargo.lock`: resolved dependencies for repeatable builds.

Dependencies: [eframe](https://docs.rs/eframe/0.33.3/eframe/),
[rfd](https://docs.rs/rfd/0.15.4/rfd/),
[walkdir](https://docs.rs/walkdir/2.5.0/walkdir/), and
[Serde](https://serde.rs/), plus bundled SQLite via
[rusqlite](https://docs.rs/rusqlite/0.40.2/rusqlite/).
