<h1 align="center">Media Shelf</h1>

<p align="center">A lightweight movie and TV library manager, built with Rust.</p>

<p align="center">
  <a href="https://github.com/alinooshabadi/media-shelf/blob/main/Cargo.toml"><img alt="Version 0.6.8" src="https://img.shields.io/badge/version-0.6.8-3865a9?style=for-the-badge" height="22"></a>
  <a href="#running-from-binaries"><img alt="Windows and macOS desktop" src="https://img.shields.io/badge/desktop-Windows%20%7C%20macOS-3865a9?style=for-the-badge" height="22"></a>
  <a href="https://www.rust-lang.org/"><img alt="Built with Rust" src="https://img.shields.io/badge/built_with-Rust-dea584?style=for-the-badge" height="22"></a>
  <a href="#configuration"><img alt="SQLite library" src="https://img.shields.io/badge/library-SQLite-4479a1?style=for-the-badge" height="22"></a>
  <a href="docs/USER_GUIDE.md"><img alt="User guide" src="https://img.shields.io/badge/docs-user_guide-4c9362?style=for-the-badge" height="22"></a>
  <a href="https://github.com/alinooshabadi/media-shelf/commits/main/"><img alt="Latest commit" src="https://img.shields.io/github/last-commit/alinooshabadi/media-shelf?style=for-the-badge" height="22"></a>
</p>

Media Shelf organizes local movie and TV collections through a minimal light interface. Add a folder, browse existing artwork and metadata, choose a TMDB match, and rename your movies into a consistent layout.

Inspired by tinyMediaManager's library and search-and-scrape workflow, with a Rust core shared by the desktop app and a headless scanner.

## Features

- **Local libraries:** recursive movie and series discovery, source management, filtering and SQLite persistence.
- **Existing media:** import supported movie NFO metadata and discover local posters and fanart without another scrape.
- **TMDB search and scrape:** preview candidates, select metadata fields and artwork, then confirm what to save.
- **Movie organization:** apply default folder and filename formats in one action, with durable Undo and overwrite protection.
- **Artwork and details:** poster-led overview, cast and crew, media files, artwork and trailer views.
- **Recoverable cleanup:** preview supported leftovers and quarantine selected files for later restoration.
- **Headless scanning:** build without desktop dependencies and output a one-time scan as JSON.

## Running from binaries

### Dependencies

- Windows x64 or a macOS build matching your Mac's architecture.
- Internet access and a TMDB API Read Access Token for online search and scraping.

SQLite is bundled. Rust and a separate database server are not required to run the Windows executable. Existing NFO and local artwork discovery work without a TMDB token.

If you have a packaged executable, run `MediaShelf-v0.6.8.exe` and:

1. Open **Data sources** and add your movie or TV folders.
2. Select an item to view its details and existing artwork.
3. Open **Settings**, enter your TMDB Read Access Token and click **Save token**.
4. Use **Search & scrape** to preview a match, choose fields and click **OK — Scrape**.
5. Right-click an item for **Search & scrape** or **Rename**. Click its displayed path to open the containing folder in Explorer.

On macOS, open **Media Shelf.app**. The same screens and workflow are shared across both platforms.

No binaries have been published to this repository's [Releases](https://github.com/alinooshabadi/media-shelf/releases) page yet. Build from source if you do not already have an executable.

## Running from source

### Dependencies

- Rust and Cargo.
- A C/C++ compiler and linker compatible with your Rust target, including support for bundled SQLite.

The Windows build has been tested with the `x86_64-pc-windows-gnu` target. Use a matching native toolchain for your target.

Clone the repository and run the desktop app:

```powershell
git clone https://github.com/alinooshabadi/media-shelf.git
cd media-shelf
cargo run --release --locked
```

To build without launching:

```powershell
cargo build --release --locked
```

On Windows, the executable is written to `target/release/media-shelf.exe`. The PowerShell build helper also produces versioned copies in the parent directory:

```powershell
.\build.ps1
```

### macOS app bundle

Install stable Rust and Apple's Xcode command-line tools (`xcode-select --install`), then run:

```sh
./build-macos.sh
open "target/macos/Media Shelf.app"
```

The script builds a native executable for the build Mac (Apple Silicon or Intel),
packages it into `target/macos/Media Shelf.app`, and applies a local ad-hoc signature.
You can copy the app to Applications. It is not Developer ID signed or notarized;
a downloaded copy may require approval in macOS Privacy & Security settings.
Building a universal binary is not included.

For an executable without an app bundle, use `cargo build --release --locked`;
the result is `target/release/media-shelf`.

## Running without the desktop

Build the shared scanner without the desktop feature:

```sh
cargo build --release --locked --no-default-features
./target/release/media-shelf --scan movies /mnt/media/Movies
./target/release/media-shelf --scan series /mnt/media/Series
```

Each command performs one recursive scan and prints JSON. It does not update the desktop database or run as a folder-watching service. Raspberry Pi builds have not been verified.

A complete scan exits with code `0`; a scan with warnings exits with code `2`.

## Movie organization

The default layout is:

```text
Movies/
└── 2001 - Fat Girl/
    ├── Fat Girl (2001).mp4
    ├── movie.nfo
    ├── poster.jpg
    └── fanart.jpg
```

**Rename applies immediately.** It uses a saved match or a readable local NFO containing a title and year. Existing sidecars move with the video when applicable; missing artwork and NFO files require scraping. File extensions are preserved.

Existing standard artwork and NFOs take priority over alternatives. Extra sidecars are retained; video and subtitle collisions stop the operation. **Rename & cleanup → Undo / recover last operation** restores the recorded paths, including after restarting.

## Configuration

On Windows, the library is stored at:

```text
%LOCALAPPDATA%\MediaShelf\library.sqlite3
```

Saved tokens are protected with Windows user-scoped DPAPI in `tmdb-token.dat` beside the database. Use **Forget token** to remove the saved copy. `TMDB_READ_ACCESS_TOKEN` is supported as a fallback when no token is saved.

On macOS, the database is stored at
`~/Library/Application Support/MediaShelf/library.sqlite3`. The token file is in
the same directory, with owner-only permissions (0600); it is not encrypted or
stored in Keychain. `TMDB_READ_ACCESS_TOKEN` also works on macOS.

Artwork caches and backups of explicitly replaced NFO/artwork files are also stored in the app's data directory. See the [user guide](docs/USER_GUIDE.md) for source behavior, scanning, output options and Linux paths.

## Development

Run the test suite:

```sh
cargo test --locked
```

The current Windows build passes 48 tests covering scanning, local artwork, NFO import/export, persistence, TMDB HTTP fixtures, rename collisions, recovery, Undo and in-process UI rendering.

On Apple Silicon macOS, `cargo test --locked` passes 48 tests and
`cargo test --locked --no-default-features` passes 46 tests. The release app
has also been checked for launch, fixture scanning and opening folders in Finder.
Intel Mac builds have not been verified.

TV scraping currently applies show metadata; individual episode scraping is not implemented. Streaming, Docker packaging, automatic folder watching and a web UI are not available.

## License

This repository does not currently include a license file.

## Documentation and credits

- [User guide](docs/USER_GUIDE.md)

Metadata and artwork are provided by [The Movie Database](https://www.themoviedb.org/).

This product uses the TMDB API but is not endorsed or certified by TMDB.
