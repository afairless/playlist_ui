# Playlist UI

A fast, cross-platform desktop application for browsing, filtering, and exporting
playlists from your local audio files. Built with Rust and
[iced](https://github.com/iced-rs/iced), it provides a file tree browser,
tag-based navigation (by genre, artist, album), and playlist export to XSPF for
use in media players like VLC.

## Features

- Browse your music library by **directory** or **tags** (genre, artist, album, track)
- Filter files by **extension** (e.g., mp3, flac, wav, etc.)
- Add files or entire directories to a **playlist panel**
- **Sort** and **shuffle** playlist entries
- Export playlists as **XSPF** (XML Shareable Playlist Format)
- **Play** exported playlists directly in VLC
- **Persistent state** (remembers your directories, sort order, and settings across restarts)
- Fast metadata scanning using [lofty](https://github.com/Serial-ATA/lofty-rs)
- Cross-platform: Linux, macOS, Windows

## Quick Start

### Prerequisites

- [Rust toolchain](https://rustup.rs/) (stable, edition 2024)
- [VLC](https://www.videolan.org/vlc/) (for the "Play" feature)
- System libraries for `iced`, `lofty`, `sled`, and `rfd` — see their
  respective docs for platform-specific requirements

### Build & Run

```sh
git clone <repository-url>
cd playlist_ui
cargo run --release
```

On first launch, the app creates two files in your home directory:

- `~/.playlist_ui_db` — Sled database caching genre and creator tag trees
- `~/.playlist_ui_top_dirs.json` — persisted list of top-level directories

## Usage

1. **Add a directory**: Click **"Add Directory"** and select a folder containing
   audio files.
2. **Browse**: Use the left panel to explore by directory structure, or switch
   to **Genre** or **Creator** tag trees using the selection-mode button.
3. **Filter**: Expand the **File Extensions** menu to toggle which file types
   (`.mp3`, `.flac`, `.wav`, etc.) are visible.
4. **Build a playlist**: Right-click files or directories and choose
   **"Add to right panel"** or **"Add all files to right panel"**.
5. **Sort / Shuffle**: Use the right panel's column headers to sort by
   directory, file name, creator, album, title, genre, or duration. Toggle
   between ascending/descending by clicking the same header again.
6. **Export**: Click **"Export to XSPF"** to save the current playlist as an
   `.xspf` file, or **"Play"** to export to a temp file and launch VLC
   immediately.
7. **Clear**: Click **"Clear Playlist"** to remove all items from the right
   panel.

### Keyboard & Interaction

- Left panel directories can be **expanded/collapsed** by clicking the folder
  node.
- Right-click a file or directory for a **context menu** with add/remove
  actions.
- Click a file row in the right panel to **open** it with the system default
  application.

## Directory Structure

```
src/
├── main.rs               — Application entry point and iced startup
├── utils.rs              — Shared utility functions (e.g., duration formatting)
├── gui/
│   ├── mod.rs            — Module re-exports and public API
│   ├── state.rs          — FileTreeApp model, Message enum, TagTreeNode, etc.
│   ├── view.rs           — View composition and styling (Elm-architecture View)
│   ├── update.rs         — Message-handling pure state transitions (Elm-architecture Update)
│   ├── left_panel.rs     — Left sidebar: directory/tag tree, extension filter
│   ├── right_panel.rs    — Right sidebar: playlist table, sorting, export controls
│   ├── render_node.rs    — Recursive tree-node rendering (FileNode, TagTreeNode)
├── fs/
│   ├── mod.rs            — Module re-exports
│   ├── file_tree.rs      — FileNode struct and recursive directory scanning
│   ├── media_metadata.rs — MediaMetadata extraction and tag-tree construction
│   ├── media_metadata_async.rs — [Experimental] async metadata extraction
│   ├── xspf.rs           — XSPF playlist export
├── db/
│   ├── mod.rs            — Module re-exports
│   ├── sled_store.rs     — Sled-based persistent store (tag trees)
docs/
├── research/             — Feature design documents and research
├── ARCHITECTURE.md       — System architecture and design decisions
AGENTS.md                 — AI-assisted development conventions
```

## Configuration

The application uses no command-line arguments or environment variables.
Configuration is handled through the UI:

- **File extension filters**: Toggled via the "File Extensions" menu in the left
  panel
- **Sort preferences**: Click column headers in the right panel
- **Top-level directories**: Added/removed via the "Add Directory" button and
  "X" remove buttons

Persistent state is stored automatically in:

| File | Purpose |
|---|---|
| `~/.playlist_ui_db` | Sled database (genre/creator tag trees) |
| `~/.playlist_ui_top_dirs.json` | Top-level directory list |

> **⚠️ Database rebuild**: The Sled database is not incrementally updated when
> file metadata changes. To refresh, delete `~/.playlist_ui_db` and restart the
> application.

## Development

### Running Tests

```sh
cargo test            # Run all tests
cargo test -- --nocapture  # Run tests with stdout visible
cargo test <test_name>      # Run a specific test
```

### Linting & Formatting

This project uses standard Rust tooling with the following conventions (defined
in `rustfmt.toml`):

```sh
cargo fmt --check     # Check formatting
cargo clippy          # Lint
```

### CI

A CI workflow runs on every push (see `.github/workflows/rust-tests.yml`). To
run the same checks locally:

```sh
cargo make ci
```

(Requires [cargo-make](https://github.com/sagiegurari/cargo-make).)

## Completion Status

This application is written for personal use and has many unfinished and
unpolished aspects compared to polished, end-user-ready software:

- **Slow first launch**: Tag-tree construction for large collections can take
  significant time on the initial run.
- **No incremental database updates**: Changes to local media files are not
  reflected until the database is deleted and rebuilt.
- **Directory picker limitations**: On some desktop environments, "Add
  Directory" may require selecting a file within the directory rather than the
  directory itself.
- **Limited playlist management**: No drag-to-reorder, bulk-delete, or manual
  reordering of playlist items.
- **Inflexible column display**: Right-panel columns auto-show/hide based on
  available metadata, but cannot be manually configured.
- **Unified tree abstraction**: The `FileNode` directory tree and `TagTreeNode`
  tag tree share similar rendering code that could be unified in a future
  refactor.

## License

This project is provided for personal use. No license is currently specified.
