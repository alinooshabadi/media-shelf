# Media Shelf

<p>
  <a href="https://github.com/alinooshabadi/media-shelf/blob/main/Cargo.toml"><img alt="Version 0.6.8" src="https://img.shields.io/badge/version-0.6.8-3865a9?style=for-the-badge" height="22"></a>
  <a href="https://github.com/alinooshabadi/media-shelf/releases"><img alt="Latest release" src="https://img.shields.io/github/v/release/alinooshabadi/media-shelf?style=for-the-badge" height="22"></a>
  <a href="#getting-started"><img alt="Windows and macOS desktop" src="https://img.shields.io/badge/desktop-Windows%20%7C%20macOS-3865a9?style=for-the-badge" height="22"></a>
  <a href="https://www.rust-lang.org/"><img alt="Built with Rust" src="https://img.shields.io/badge/built_with-Rust-dea584?style=for-the-badge" height="22"></a>
  <a href="#configuration"><img alt="SQLite library" src="https://img.shields.io/badge/library-SQLite-4479a1?style=for-the-badge" height="22"></a>
  <a href="docs/USER_GUIDE.md"><img alt="User guide" src="https://img.shields.io/badge/docs-user_guide-4c9362?style=for-the-badge" height="22"></a>
  <a href="https://github.com/alinooshabadi/media-shelf/commits/main/"><img alt="Latest commit" src="https://img.shields.io/github/last-commit/alinooshabadi/media-shelf?style=for-the-badge" height="22"></a>
</p>

A desktop app for organizing local movie and TV collections on Windows and macOS, built with Rust and egui.

[User guide](docs/USER_GUIDE.md) · [Releases](https://github.com/alinooshabadi/media-shelf/releases)

## Features

- Browse sortable movie libraries and expandable TV show, season and episode trees.
- Track added dates and optionally display missing seasons and episodes from TMDB.
- Search TMDB for metadata, posters, fanart, cast and crew.
- Import local movie NFOs and artwork.
- Organize folders, videos and subtitles with overwrite protection and Undo.
- Preview and quarantine leftover files for recoverable cleanup.
- Scan libraries from the command line and export JSON.

## Getting started

1. Launch the Windows executable or **Media Shelf.app** on macOS.
2. Open **Data sources** and add your movie and series folders.
3. For online metadata, enter a TMDB API Read Access Token in **Settings** and click **Save token**.
4. Select an item and use **Search & scrape** to choose a match and the fields to save.

Local scanning works without a TMDB token. SQLite is bundled; no database server is needed.

## Build from source

Requires Rust, Cargo and a C/C++ toolchain. On macOS, install Xcode command-line tools with `xcode-select --install`.

```sh
git clone https://github.com/alinooshabadi/media-shelf.git
cd media-shelf
cargo run --release --locked
```

To build without launching, use `cargo build --release --locked`. The executable is written to `target/release/`.

### macOS app bundle

```sh
./build-macos.sh
open "target/macos/Media Shelf.app"
```

The bundle targets the build Mac's architecture and can be copied to Applications. It is locally signed, but not notarized by Apple.

### Windows build

```powershell
.\build.ps1
```

The script builds the executable and creates versioned copies in the parent directory.

## Library organization

Movies are scanned recursively. Series indexing lists only first-level show folders to keep network library scans fast; episodes load when you expand or rename a selected show.

Default layouts:

```text
Movies/2001 - Fat Girl/Fat Girl (2001).mp4
Series/Show (2020)/Season 1/Show - S01E01 - Pilot.mkv
```

Movie renaming uses a saved match or local NFO with a title and year. Series renaming uses the matched show and local episode titles, fetching missing titles from TMDB. Subtitles, NFOs and thumbnails follow the video names.

**Rename applies immediately.** Existing files are never overwritten. Use **Rename & cleanup → Undo / recover last operation** to restore original paths, including after restarting.

## Command-line scanning

Build without the desktop UI:

```sh
cargo build --release --locked --no-default-features
./target/release/media-shelf --scan movies /path/to/Movies
./target/release/media-shelf --scan series /path/to/Series
```

Each command prints JSON without updating the desktop library. Exit code `0` indicates a complete scan; `2` indicates scan warnings.

## Configuration

The library database is stored at:

| Platform | Path |
| --- | --- |
| Windows | `%LOCALAPPDATA%\MediaShelf\library.sqlite3` |
| macOS | `~/Library/Application Support/MediaShelf/library.sqlite3` |

Save your TMDB token in Settings, or set `TMDB_READ_ACCESS_TOKEN`. See the [user guide](docs/USER_GUIDE.md) for token storage, backups and other configuration details.

## Development

```sh
cargo test --locked
```

## Credits

Inspired by tinyMediaManager. Metadata and artwork are provided by [The Movie Database](https://www.themoviedb.org/).

This product uses the TMDB API but is not endorsed or certified by TMDB.
