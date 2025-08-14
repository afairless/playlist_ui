# Playlist UI

A fast, cross-platform desktop application for browsing, filtering, and exporting playlists from your local audio files. Built with Rust and [iced](https://github.com/iced-rs/iced), it provides a file tree browser, tag-based navigation (by genre, artist, album), and playlist export to XSPF for use in media players like VLC.

## Features

- Browse your music library by directory or tags (genre, artist, album, track)
- Filter files by extension (e.g., mp3, flac, wav, etc.)
- Add files or entire directories to a playlist panel
- Sort and shuffle playlist entries
- Export playlists as XSPF (XML Shareable Playlist Format)
- Play exported playlists directly in VLC
- Persistent state (remembers your directories and settings)
- Fast metadata scanning using [lofty](https://github.com/Serial-ATA/lofty-rs)
- Cross-platform: Linux, macOS, Windows

## Installation

### Prerequisites

- [Rust toolchain](https://rustup.rs/) (stable)
- [Node.js](https://nodejs.org/) (for some iced backends, optional)
- [VLC](https://www.videolan.org/vlc/) (for "Play" feature)
- System libraries for `iced`, `lofty`, `sled`, and `rfd` (see their docs)

### Build

Clone the repository:

```sh
git clone https://github.com/afairless/playlist-ui.git
cd playlist-ui
```

Build and run:

```sh
cargo run --release
```

### Run

The app will create a `.playlist_ui_db` and `.playlist_ui_top_dirs.json` in your home directory for persistent state.

## Usage

1. **Add Directory**: Click "Add Directory" to select a folder containing your audio files.
2. **Browse**: Use the left panel to browse by directory or switch to tag-based navigation (Genre/Creator).
3. **Filter**: Use the file extension menu to filter which file types are shown.
4. **Build Playlist**: Add files or directories to the right panel (playlist) via context menus or buttons.
5. **Sort/Shuffle**: Use the right panel controls to sort or shuffle your playlist.
6. **Export/Play**: Export the playlist as XSPF or play it directly in VLC.

## File/Folder Structure

- `src/main.rs` — Application entry point
- `src/gui/` — UI logic (panels, rendering, state, update)
- `src/fs/` — Filesystem and metadata utilities
- `src/db/` — Sled-based persistent storage
- `src/utils.rs` — Utility functions

## Development

### Running Tests

```sh
cargo test
```

### Completion Status

This project is written for personal use and has many unfinished and unpolished aspects compared to well-written, end-user-ready software.  These include:

- Initial start-up is very slow for large media file collections, as the application is building the database for the first time
- Incremental updates are not implemented for the sled database.  Thus, any changes to local media files will not be captured by the database; one must delete the database file and restart the application, so that the database is rebuilt from scratch.
- Depending on one's local desktop environment, "Add Directory" may not allow selection of a directory directly.  Instead, one must select a file within the desired directory.
- There are limited options for deleting multiple files from the playlist (though one can delete all files within a directory)
- Display of playlist columns is inflexible and may display some items poorly
- One cannot manually and arbitrarily re-order individual items in the playlist
- The internal file browser interface for the file system tree and the media tag trees should probably be unified (which is one of several potential refactorings/improvements to the internal code organization)
