# Verification

## v0.6.8 — Duplicate planned sidecar destinations

- All 48 release-mode tests and the Windows build passed.

- Standard movie sidecar planning now detects targets already reserved by another sidecar, not only existing files. Generic movie.nfo/poster/fanart files are processed first with stable filename ordering; alternatives remain at their source paths.
- Video/subtitle collisions and unexpected apply-time destinations remain errors. No overwritten or deleted sidecars are introduced.
- Added a [REC]² fixture with generic and release-named fanart/poster/NFO pairs targeting a new movie folder, validating preferred contents, retained alternatives and Undo. No user media files were modified during verification.
## v0.6.7 — Clickable folder paths

- All 47 existing release-mode tests and the Windows build passed. Native Explorer launch was not exercised.

- Paths in Overview, Media files, TV show folders and Data sources open the corresponding folder in Windows Explorer. Media file paths open their parent directory. Extended drive/UNC prefixes are normalized through the existing display-path helper.
- Explorer is launched directly with a folder argument, without a shell command. Unavailable folders and launch failures report an error. Automated tests do not open Explorer.
## v0.6.6 — Existing local movie NFO import and rename

- Imports supported local movie.nfo/video-stem.nfo metadata for unmatched movies at startup and after scans. TMDB IDs create real persisted matches; title/year-only NFOs enable rename directly without fake IDs or a network scrape.
- Preserves existing app metadata and reads NFO bytes without modifying them. Generic NFOs are not assigned across multi-video directories. Invalid/oversized/DTD XML is rejected. TV NFO import and preservation of unsupported fields during explicit NFO replacement remain unimplemented.
- All 47 release-mode tests and the Windows build passed. New fixtures cover TMM import, UTF-8/entity parsing, SQLite reopen, rename/Undo, original NFO byte preservation, custom metadata priority, title-only local renaming and ambiguous/invalid input.
- The running generic MediaShelf.exe could not be replaced; the versioned MediaShelf-v0.6.6.exe was produced successfully.
## v0.6.5 — Existing standard artwork/NFO collisions

- All 44 release-mode tests and the Windows build passed.

- Movie organization preserves existing destination artwork/NFO files and retains alternative source sidecars instead of aborting. Same existing paths, including Windows case aliases, are treated as no-ops. Video and subtitle destination collisions remain errors; apply-time collision detection is unchanged.
- Read-only inspection of the reported Night and Fog folder confirmed generic and filename-based fanart/poster/NFO pairs. No user files were changed by the agent.
- Added a fixture reproducing those pairs, checking video/subtitle renames, byte-for-byte preservation of both sidecar versions, and Undo.
## v0.6.4 — Existing local poster and fanart discovery

- All 43 release-mode tests and the Windows build passed.

- Loads generic and video-specific JPG/JPEG/PNG/WebP posters and fanart at scan/startup/library refresh, without a TMDB match. Directory listings are reused across files. Episode artwork also falls back to the show directory.
- Local artwork is used in Overview, Artwork and the No artwork filter. Existing cached/remote artwork remains a fallback. Added extended UNC path support to local image URIs.
- Tests cover case-insensitive generic images, provider-free discovery, database reopen, removed images, filename-specific priority and show-folder artwork. No user artwork files are written or modified by discovery. Native visual verification was not performed.
## v0.6.3 — Folder-derived movie names and sample exclusions

- All 41 release-mode tests and the Windows build passed.

- Unmatched movie names/years come from source-relative YYYY - Title folders before cleaned filenames. Library display and TMDB suggested searches share the same resolver; confirmed matches retain priority.
- Scanner excludes TMM-deleted folders, sample directories and sample-suffix clips. A clean online rescan prunes old entries; no media files are modified.
- Added fixtures for numeric movie titles, folder priority, release suffixes, year extraction and scanner exclusions. Native visual interaction with the user's I: drive was not performed. NFO import remains unsupported.
## v0.6.2 — Supplied TMM movie NFO format

- Movie exporter follows the supplied sample tag structure, legacy IDs, IMDb-default unique ID, themoviedb ratings, outline/certification, producer blocks, languages and Kodi trailer syntax. Artwork uses HTTPS; creator comment truthfully identifies Media Shelf. TV export remains unchanged.
- Added backwards-compatible actor/crew IDs and photo paths, plus first-scrape original filename retention. Unknown technical/playback/application values are empty rather than invented. Existing files are not modified automatically; prior replacement/backup policy remains.
- All 39 release-mode tests and the Windows build passed. New XML tests cover the sample's top-level fields, escaping, person links, producer output, legacy actor JSON, trailer parsing, ID defaults and empty technical fields.
- The user's Z: sample was read only, and was not modified. Existing NFO import/unsupported-field preservation and media probing remain unavailable.
## v0.6.1 — Immediate default rename and standard movie sidecars

- Rename now calculates and applies the standard plan immediately, with no editor/confirmation or success popup. Errors remain visible. Cleanup and Undo retain their previews.
- Movie NFO/artwork names become movie.nfo, poster.ext and fanart.ext. Existing generic artwork moves when owned by a single-video folder. Correctly named videos still normalize legacy sidecars. Organized movie re-scrapes use generic output names.
- All 38 release-mode tests and the Windows build passed. Coverage includes standard sidecar names, restart/Undo, already-correct movies, generic artwork relocation and re-scrape NFO naming. Image formats/extensions are preserved; rename does not invent missing assets.
## v0.6 — Minimal light interface

- Added a consistent light palette, Windows system typography, larger row targets, clear status labels, rounded artwork, poster-led overview, compact metadata badges, wrapped studio text and direct scrape/rename actions.
- Removed duplicate fanart from the overview; artwork, cast, file, trailer and tags views remain available. Existing persistence and file-operation behavior is retained.
- 36 release-mode tests passed. New in-process checks cover every detail tab at 1080/1380 widths and long studio-name wrapping. The chooser test uses the new theme and verifies the apply button remains visible at normal and short heights. Its footer reserve was adjusted for the larger type.
- Native visual click-through was not performed; these checks exercise egui rendering in process.
## v0.5.4 — Movie folder organization

- Movie rename previews now target Year - Title/Title (Year).ext; original extensions are preserved. Movies in source roots get a new subfolder; movies in existing folders get a sibling folder. Series retain their current layout.
- Related named sidecars move with the video. Original directories and unrelated files remain in place. Missing matches/years, redirected directories and destination collisions stop the preview/apply. No existing files are overwritten.
- All 35 release-mode tests and the Windows build passed. New fixture tests cover root and existing-folder movies, exact Fat Girl (2001) naming, sidecars, SQLite path updates, repeat no-op previews, restart/Undo, collisions and series layout.
## v0.5.3 — Persistent TMDB token

- All 33 release-mode tests and the Windows build passed.

- Added explicit Save token / Forget token settings actions and automatic startup loading. A saved token takes precedence over the environment variable; the environment remains a fallback when no token is saved.
- Windows uses user-scoped DPAPI ciphertext in the user-local tmdb-token.dat file. Writes replace atomically. Failed reads/saves display a message without logging token contents. Linux uses owner-only file permissions.
- Added real Windows DPAPI tests with generated dummy tokens: save/load, replacement, deletion, ciphertext not containing the token, and corrupt-input handling. No user token was read by these persistence tests.
## v0.5.2 — Item context actions

- Added Search & scrape and Rename context actions to movie rows, show headers and episode rows, using the existing dialogs and rename previews.
- Right-click preserves an existing selection or selects the clicked item/group. Actions are disabled during scans and other dialogs/operations. Multi-file rename uses the existing matched-file batch preview.
- All 31 existing release-mode tests and the Windows build passed. Native right-click interaction was not visually verified.
## v0.5.1 — Visible scrape confirmation

- Constrained the preview divider and placed the scrollable dialog body above a fixed action footer.
- Regression test verifies OK — Scrape text is inside the visible clip and viewport for movies and shows at 880-pixel height, and movies at 600-pixel height. Rendering does not commit the selection.
- Release-mode chooser test and Windows executable build passed. No native UI capture was performed.
## v0.5 — Library/details and search-to-scrape flow

- Windows optimized build passed and the executable launched against an isolated sample library.
- Existing sample database migrated to schema version 4; SQLite integrity_check returned ok and saved metadata remained readable.
- All 31 release-mode tests passed, including an in-process rendering check for movie and TV choosers that confirms preview rendering does not commit data. Thirty backend tests passed, covering discovery, HTTP search/details fixtures, persistence, field merging, missing-only updates, XML/NFO output and backups, TV-show scope, rename/undo, and metadata survival after rescanning.
- Authentication against the live TMDB API was not tested because no user token was supplied. HTTP tests use local mock servers; media changes use generated fixtures.
- Native UI capture was rejected by the computer-use approval check. A visual click-through could not be completed; do not treat this as visually verified.
- TV scraping currently saves show metadata across its files and writes tvshow.nfo. Individual episode titles/plots and episode NFOs are not implemented.

## v0.4 — Rename and recoverable cleanup

- 22 tests passed: five scanner/API tests, eight persistence tests, and nine
  filesystem-operation tests, all on Windows x64.
- Tests exercise filename suggestions, multi-episode markers, related sidecars,
  generic artwork preservation, database/TMDB match updates, duplicate targets,
  existing destinations, invalid names/extensions, changed files, cleanup
  selection, quarantine exclusion, restoration after reopening the database,
  database-failure rollback, interrupted-operation recovery, collision/changed
  target refusal during Undo, and one physical video in both categories.
- Operations are tested only against generated temporary fixtures. No user's
  media folders were renamed or cleaned by the agent.
- Database schema version 3 adds an operation journal. Windows moves use
  `MoveFileExW` without replacement/cross-volume-copy flags.
- Optimized Windows build and formatting check passed. The new executable
  launched against an isolated v0.3 test database, upgraded to schema version 3,
  created the operation journal, and retained one movie and two episode records.
  SQLite integrity check returned `ok`.
- Folder relocation/renaming, permanent deletion, individual TMDB episode lookup,
  and Pi builds are outside this version. UI click-through remains unverified.

## v0.3 — TMDB search and manual matching

- Thirteen tests passed: five scanner/API unit tests and eight SQLite integration
  tests. API requests are tested against a local HTTP server, without real tokens.
- Verified movie/TV endpoint selection, UTF-8 query encoding, header-based bearer
  authentication, paging, result mapping, missing dates, empty results, malformed
  JSON, and helpful errors for 401/403/429/503 without exposing response bodies
  or credentials.
- Verified chosen matches survive close/reopen and rescan, can be replaced or
  cleared, reject category mismatches and missing files, and are removed when
  the video leaves the library. Tested migration from a v0.2 database without
  losing the source list or cached video records.
- Credentials are session-only, or read from `TMDB_READ_ACCESS_TOKEN`; they are
  not stored in the database. The TMDB logo is embedded locally.
- Optimized Windows build and formatting check passed. The versioned executable
  launched against an isolated copy of the v0.2 test database, upgraded its schema
  to version 2, and preserved one movie and two episode records. SQLite integrity
  check returned `ok`. The user's running older executable was left open.
- Live authenticated TMDB search has not been tested because no user token was
  supplied. Windows UI click-through testing and Raspberry Pi builds remain
  unverified.

## v0.2 — SQLite persistence

Verified on Windows x64, 20 October 2026.

- Eight tests passed: the original two scanner tests and six file-backed SQLite
  integration tests.
- Database tests cover close/reopen persistence, Unicode paths, movie/episode
  separation, changed sizes, deletion reconciliation, duplicate prevention,
  timestamps, offline drives, cancellation, source removal with overlap,
  one-time JSON migration, retry after invalid JSON, transaction rollback, and
  rejection of scans using outdated source selections.
- Desktop loads the cached library on startup and refreshes it on demand.
- Rebuilt v0.2 executable launched with an isolated profile, created its SQLite
  file, and imported movie/series folders from legacy JSON. An independently
  seeded library containing one movie and two episodes survived a desktop
  process restart; SQLite `integrity_check` returned `ok` before and after.
- Rust formatting check passed and the executable has no SQLite DLL import.
- SQLite 3 is bundled via rusqlite 0.40.2, with no external SQLite DLL required.
- Raspberry Pi compilation and UI click-through testing remain unverified.

## v0.1 — Initial scanner

Verified on Windows x64, 20 October 2026.

- Built the optimized desktop executable with Rust 1.99.1 (Windows GNU target),
  eframe 0.33.3, and rfd 0.15.4.
- Two scanner tests passed: recursive movie/episode discovery, case-insensitive
  extensions, exclusion of image/partial files, overlapping-root deduplication,
  missing-root reporting, and cancellation.
- Built the separate headless feature configuration.
- Command-line smoke check returned one movie and two nested series episodes.
- Confirmed exit status 2 for an unavailable root and 1 for invalid arguments.
- Checked Rust formatting.
- The desktop executable launched and exposed a Windows window titled
  `Media Shelf`. Its import table references Windows system DLLs.
- Automated click-through and visual verification were blocked by the
  computer-use approval check. Folder-picker interaction, layout, and settings
  restoration have not been verified through the UI.
- Raspberry Pi/Linux compilation and execution have not been tested.

The build tools were kept under the workspace's `work/toolchain` directory;
the user does not need them to run `MediaShelf.exe`. The portable GCC distribution
combines exception-unwind code into `libgcc.a`; a local `libgcc_eh.a` alias was
provided for the Rust GNU linker. No system compiler configuration was changed.
