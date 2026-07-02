# Playlist UI — Architecture

## System Overview

Playlist UI is a cross-platform desktop application built with
[iced 0.13](https://github.com/iced-rs/iced), following the **Elm
architecture**:

- **Model** (`state.rs`): `FileTreeApp` — a single struct holding all
  application state
- **Update** (`update.rs`): `update()` — a pure state-transition function that
  reacts to `Message` variants
- **View** (`view.rs`): `view()` — a pure layout function that produces an
  `Element<Message>` widget tree from the current state

The application presents a two-panel layout:

| Panel | Purpose |
|---|---|
| **Left panel** | Directory tree or tag-based tree (genre/creator) with extension filtering |
| **Right panel** | Playlist (sortable table) with shuffle, export, and play controls |

## Module Map

```
main.rs
├── gui/                  ← Elm-architecture UI
│   ├── state.rs          ← FileTreeApp (Model), Message (Action enum)
│   ├── update.rs         ← update() — pure state transitions
│   ├── view.rs           ← view() — layout composition, style constants
│   ├── left_panel.rs     ← Left sidebar assembly
│   ├── right_panel.rs    ← Right panel assembly
│   └── render_node.rs    ← Recursive tree rendering + colour highlights
├── fs/                   ← Filesystem operations
│   ├── file_tree.rs      ← FileNode struct + scan_directory()
│   ├── media_metadata.rs ← MediaMetadata + tag tree builders
│   ├── media_metadata_async.rs ← [Experimental, not wired]
│   └── xspf.rs           ← XSPF playlist export
├── db/
│   └── sled_store.rs     ← SledStore (persistent key-value for tag trees)
└── utils.rs              ← format_duration()
```

## Data Flow

```
User action ──→ Message ──→ update(&mut FileTreeApp, Message) ──→ Task<Message>
                                  │                                      │
                                  ▼                                      ▼
                            FileTreeApp (mutated)              Side effects
                                  │                           (file dialogs,
                                  ▼                            export, etc.)
                            view(&FileTreeApp) ──→ Element<Message> ──→ Screen
```

### Initialisation (`main.rs`)

1. **Sled DB** is opened at `~/.playlist_ui_db`.
2. **Top directories** are loaded from `~/.playlist_ui_top_dirs.json` (JSON
   array of `PathBuf`).
3. **Genre and creator tag trees** are built on first launch (when the sled
   database is empty) via `build_genre_tag_tree` and `build_creator_tag_tree`,
   then cached in Sled.
4. `FileTreeApp::load()` restores persisted dirs, creates a `FileTreeApp` with
   `FileNode` trees from `scan_directory()`.
5. `iced::application().run_with()` starts the event loop with the initial
   state.

### Message Flow

The `Message` enum (defined in `state.rs`) has variants for every user action:

| Category | Messages |
|---|---|
| **Navigation** | `ToggleLeftPanel`, `ToggleLeftPanelSelectMode`, `ToggleLeftPanelSortMode` |
| **File tree** | `ToggleExpansion`, `ToggleExtension`, `ToggleExtensionsMenu` |
| **Tag tree** | `ToggleTagExpansion`, `AddTagNodeToRightPanel` |
| **Directories** | `AddDirectory`, `DirectoryAdded`, `RemoveTopDir` |
| **Playlist** | `AddToRightPanel`, `AddDirectoryToRightPanel`, `RemoveFromRightPanel`, `RemoveDirectoryFromRightPanel`, `ClearRightPanel` |
| **Sorting** | `SortRightPanelBy*` (7 variants), `ShuffleRightPanel` |
| **Export** | `ExportRightPanelAsXspf`, `ExportRightPanelAsXspfTo`, `ExportAndPlayRightPanelAsXspf` |
| **Open** | `OpenRightPanelFile` |

Every update arm is a pure, synchronous state transition. Side effects use
`Task::perform()`:

- `FileDialog::pick_folder()` → `DirectoryAdded(Option<PathBuf>)`
- `FileDialog::save_file()` → `ExportRightPanelAsXspfTo(PathBuf)`
- `Command::new("vlc").spawn()` is a fire-and-forget side effect inside the
  `ExportAndPlayRightPanelAsXspf` handler

## Key Data Structures

### `FileTreeApp` (Model)

```rust
struct FileTreeApp {
    // Navigation
    left_panel_selection_mode: LeftPanelSelectMode,  // Dir | Genre | Creator
    left_panel_expanded: bool,
    left_panel_sort_mode: LeftPanelSortMode,          // Alpha | DateModified
    top_dirs: Vec<PathBuf>,                           // persisted
    root_nodes: Vec<Option<FileNode>>,                // rebuilt on change

    // Tag trees
    tag_tree_roots: Vec<TagTreeNode>,                 // genre or creator tree
    sled_store: Option<SledStore>,                    // persistence handle

    // Filters
    selected_extensions: Vec<String>,
    all_extensions: Vec<String>,
    extensions_menu_expanded: bool,

    // Expansion state
    expanded_dirs: HashSet<PathBuf>,

    // Playlist (right panel)
    right_panel_files: Vec<RightPanelFile>,
    right_panel_sort_column: SortColumn,
    right_panel_sort_order: SortOrder,
    right_panel_shuffled: bool,
}
```

The `top_dirs` and `right_panel_sort_column`/`right_panel_sort_order` fields are
serialised via serde JSON to `~/.playlist_ui_top_dirs.json`.

### `FileNode` (Directory Tree)

```rust
struct FileNode {
    name: String,
    path: PathBuf,
    node_type: NodeType,       // File | Directory
    children: Vec<FileNode>,
    is_expanded: bool,
    file_count: usize,         // recursive count of audio-file descendants
}
```

Built by `scan_directory()` in `file_tree.rs`. Nodes marked `NodeType::File`
represent matching audio files; `NodeType::Directory` nodes group their
children. The `file_count` field is computed during construction:
`new_file()` sets it to 1, `new_directory()` sums children's counts.

### `TagTreeNode` (Tag Tree)

```rust
struct TagTreeNode {
    label: String,                                    // genre, artist, album, or track title
    children: Vec<TagTreeNode>,
    file_paths: Vec<PathBuf>,                         // only leaf nodes have file paths
    is_expanded: bool,
    file_count: usize,                                // recursive count of track descendants
}
```

Built by `build_genre_tag_tree()` and `build_creator_tag_tree()` in
`media_metadata.rs`. The hierarchy is:

- **Genre mode**: Genre → Artist → Album → Track
- **Creator mode**: Artist → Album → Track

Leaf nodes have exactly one `file_path` entry. Non-leaf nodes compute
`file_count` as the sum of their children's counts.

### `RightPanelFile` (Playlist Entry)

```rust
struct RightPanelFile {
    path: PathBuf,
    creator: Option<String>,
    album: Option<String>,
    title: Option<String>,
    genre: Option<String>,
    duration_ms: Option<u64>,
}
```

Populated from `MediaMetadata` when a file is added to the playlist. Not
persisted across restarts.

## Persistence

The application uses two persistence mechanisms:

| Mechanism | What it stores | Where | Format |
|---|---|---|---|
| **JSON file** | `top_dirs`, `right_panel_sort_column`, `right_panel_sort_order` | `~/.playlist_ui_top_dirs.json` | JSON (serde) |
| **Sled DB** | Genre and creator tag trees (cached) | `~/.playlist_ui_db` | Bincode-encoded `Vec<TagTreeNode>` |

The sled database is **not incrementally updated**. If file metadata changes,
the database must be deleted and rebuilt on the next launch.

## Rendering Pipeline

```
view(&FileTreeApp)
  ├── create_left_panel()
  │     ├── create_left_panel_menu_row()        ← Add Directory, sort toggle
  │     ├── create_extension_menu()             ← File extension toggles
  │     ├── create_left_panel_file_tree_browser() ← FileNode rendering
  │     │     └── render_file_node()            ← recursive, depth-indented
  │     │           ├── NodeType::Directory → expand/collapse + file_count
  │     │           └── NodeType::File      → add-to-playlist context menu
  │     └── create_left_panel_tag_tree_browser() ← TagTreeNode rendering
  │           └── render_tag_node()             ← recursive, depth-indented
  │
  └── create_right_panel()
        ├── create_right_panel_menu_row()       ← Shuffle, Export, Play, Clear
        ├── create_totals_display()             ← Item count + total duration
        ├── create_right_panel_header_row()     ← Sortable column headers
        └── create_right_panel_file_rows()      ← Alternating row colours
              └── ContextMenu per cell          ← Delete actions
```

### Colour Highlights

Directory and tag-tree categories use log-scale colour interpolation for
their background tint. The `file_count_highlight()` function in `render_node.rs`
maps a node's file count to a colour ranging from faint blue (few files) to
deep navy blue (many files), normalised logarithmically against the maximum
count in the current tree view.

## XSPF Export Pipeline

```
sorted_right_panel_files()
  → filter by audio extensions
  → export_xspf_playlist(files, output_path)
      → extract_media_metadata() for each file
      → build XML trackList with <location>, <title>, <creator>, etc.
      → write to file
```

The export preserves the user's current sort order. The **"Play"** action
writes to a temp file and spawns VLC with the playlist.

## Design Decisions

### 1. Elm Architecture over immediate-mode GUI

Iced's Elm architecture provides deterministic, testable state transitions.
Every `update` arm can be unit-tested by constructing a `Message` and asserting
on the resulting `FileTreeApp` state. This is reflected in the extensive test
suite in `state.rs` and `update.rs`.

### 2. Tag trees cached in Sled over on-demand building

Building genre and creator tag trees requires scanning every audio file and
extracting metadata, which is I/O-bound and slow for large collections. The
trees are built once and cached in Sled so subsequent launches load instantly.
Trade-off: the cache is not incrementally updated.

### 3. File count computed once during tree construction

Instead of re-traversing trees at render time, `file_count` is computed in
`FileNode::new_directory()` and during tag-tree construction, stored as a field
on every node. This avoids repeated recursion and makes the count available
for colour highlighting without additional traversal.

### 4. Per-tree max for colour normalisation

The maximum `file_count` is computed per top-level tree view (directory tree or
tag tree) rather than globally across all trees. This keeps colour intensity
perceptually meaningful within the current browsing context.

### 5. Context menus from `iced_aw`

The project uses `iced_aw::ContextMenu` for right-click actions on tree nodes
and playlist rows. These closures capture state by value, requiring `clone()`
of captured data inside the closure to avoid ownership issues.

## Research Documents

Feature and design research is stored in `docs/research/`:

- [`file-count-indicator-in-left-panel.md`](./research/file-count-indicator-in-left-panel.md) —
  Design and implementation of the file-count label and dynamic background
  highlight feature

## Future Considerations

- **Unified tree abstraction**: `FileNode` (directory tree) and `TagTreeNode`
  (tag tree) share similar rendering and interaction patterns. A common trait
  or enum would reduce code duplication in `render_node.rs` and
  `left_panel.rs`.
- **Incremental DB updates**: Watch filesystem changes and update the Sled
  database incrementally rather than requiring a full rebuild.
- **Drag-and-drop**: Manual reordering of playlist items via drag-and-drop.
- **Column configuration**: Allow users to show/hide/reorder right-panel columns.
- **Async metadata extraction**: The `media_metadata_async.rs` module exists but
  is not wired into the update path. If metadata extraction becomes a
  bottleneck, this module can be connected.
- **Benchmarking**: Large collections (10,000+ files) may benefit from
  profiling and optimisation of the tag-tree construction and serialisation
  paths.
