# Text Search Feature for the Left Panel

## Overview

Add a text search box to the left panel that lets users filter files or tag
nodes by a case-insensitive string. The search has seven configurable modes:
directory path, track filename, each individual MP3 tag field (creator,
album, title, genre), or all fields combined.

The search box sits in its own row between the "Select by" / "File Extensions"
menu row (row 2) and the tree browser, inside the left panel's `column![]`.

---

## 1. New Types and Enum

### 1.1 `TextSearchMode` enum (in `state.rs`)

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextSearchMode {
    All,             // search all fields below
    DirectoryPath,   // match against parent directory path
    TrackFilename,   // match against file name + extension
    Creator,         // match against MP3 artist/creator tag
    Album,           // match against MP3 album tag
    Title,           // match against MP3 title tag
    Genre,           // match against MP3 genre tag
}
```

### 1.2 New fields on `FileTreeApp` (in `state.rs`)

```rust
#[serde(skip)]
pub search_query: String,

#[serde(skip)]
pub search_mode: TextSearchMode,
```

Initialise both in `FileTreeApp::new()`:

- `search_query: String::new()`
- `search_mode: TextSearchMode::All`

### 1.3 New `Message` variants (in `state.rs`)

```rust
SearchQueryChanged(String),
ToggleSearchMode,
```

- `SearchQueryChanged(query)` — fired by the text input widget on every
  keystroke. The `update()` handler stores the new string.
- `ToggleSearchMode` — cycles `All → DirectoryPath → TrackFilename →
  Creator → Album → Title → Genre → All`.

---

## 2. Update Handler (in `update.rs`)

### 2.1 `Message::SearchQueryChanged(query)`

```rust
Message::SearchQueryChanged(query) => {
    app.search_query = query;
    Task::none()
}
```

Pure state — no side effects. The filtering is applied lazily in `view()` /
`left_panel.rs` at render time.

### 2.2 `Message::ToggleSearchMode`

```rust
Message::ToggleSearchMode => {
    app.search_mode = match app.search_mode {
        TextSearchMode::All => TextSearchMode::DirectoryPath,
        TextSearchMode::DirectoryPath => TextSearchMode::TrackFilename,
        TextSearchMode::TrackFilename => TextSearchMode::Creator,
        TextSearchMode::Creator => TextSearchMode::Album,
        TextSearchMode::Album => TextSearchMode::Title,
        TextSearchMode::Title => TextSearchMode::Genre,
        TextSearchMode::Genre => TextSearchMode::All,
    };
    Task::none()
}
```

### 2.3 Export the new items

Add `TextSearchMode` to the re-exports in `gui/mod.rs` and mark new public
types/functions in docstrings.

---

## 3. Filtering Logic

The filtering happens **at render time** in two places:

### 3.1 File tree filtering (in `left_panel.rs`)

A private function that filters a `FileNode` tree according to the current
`search_query` and `search_mode`:

```rust
/// Recursively filters a `FileNode` tree, keeping only nodes that match
/// the current search. Returns `None` when no descendant matches.
fn filter_file_node(
    node: &FileNode,
    query: &str,
    mode: TextSearchMode,
    top_dirs: &[PathBuf],
) -> Option<FileNode>;
```

**Matching rules** (case-insensitive, `to_ascii_lowercase`):

| Mode            | Match against                                          |
|-----------------|--------------------------------------------------------|
| `All`           | Any of the six checks below                            |
| `DirectoryPath` | `node.path.to_string_lossy()` (full path, case-insensitive) contains q |
| `TrackFilename` | `node.path.file_name()` contains q                     |
| `Creator`       | Extract metadata via `extract_media_metadata`, check   |
|                 | `creator` field                                        |
| `Album`         | Same, check `album` field                              |
| `Title`         | Same, check `title` field                              |
| `Genre`         | Same, check `genre` field                              |

**Empty query** → no filtering (all nodes pass).

**Directory nodes** that match the query by name/path are kept even if their
children don't match (they are visible as a match). Directory nodes that do
NOT match by name are kept only if at least one descendant matches (so the
tree structure is preserved to the matching leaf).

**Metadata modes (`Creator`, `Album`, `Title`, `Genre`) on file trees:**
For every file node encountered, call `extract_media_metadata(&node.path)`
and check only the relevant field. To avoid repeated extraction when the
tree or extensions haven't changed, maintain a metadata cache on
`FileTreeApp` (`HashMap<PathBuf, MediaMetadata>`) populated lazily on the
first search in a metadata mode and invalidated when extensions change.
See Section 6.1 for details.

The file-metadata-checking step should be extracted into a separate helper
so it can be unit-tested independently with hardcoded `MediaMetadata`:

```rust
fn file_matches_metadata_mode(path: &Path, mode: TextSearchMode) -> bool {
    let meta = extract_media_metadata(path);
    matches_mode(&meta, mode)
}
```

### 3.2 Tag tree filtering (in `left_panel.rs`)

A similar function for `TagTreeNode`:

```rust
fn filter_tag_node(
    node: &TagTreeNode,
    query: &str,
    mode: TextSearchMode,
) -> Option<TagTreeNode>;
```

**Matching rules:**

| Mode            | Match against                                          |
|-----------------|--------------------------------------------------------|
| `All`           | Any of the six below                                   |
| `DirectoryPath` | Any `file_path` in the subtree contains query          |
| `TrackFilename` | Any `file_path` filename in subtree contains query     |
| `Creator`       | `node.label` contains query, or any file in the subtree|
|                 | has matching `creator` tag                             |
| `Album`         | Same, check `album` tag                                |
| `Title`         | Same, check `title` tag                                |
| `Genre`         | Same, check `genre` tag                                |

### 3.3 Applying the filter in `create_left_panel`

Before constructing the tree browser, check `app.search_query`:

```rust
let tree_browser = match app.left_panel_selection_mode {
    LeftPanelSelectMode::Directory => {
        let filtered_roots: Vec<Option<FileNode>> = if app.search_query.is_empty() {
            app.root_nodes.clone()
        } else {
            app.root_nodes.iter().map(|opt| {
                opt.as_ref().and_then(|node| {
                    filter_file_node(node, &app.search_query, app.search_mode, &app.top_dirs)
                })
            }).collect()
        };
        create_left_panel_file_tree_browser_from_nodes(
            &filtered_roots,
            &app.top_dirs,
            tree_browser_style,
            flat_button_style,
            app.left_panel_sort_mode,
        )
    },
    LeftPanelSelectMode::GenreTag | LeftPanelSelectMode::CreatorTag => {
        let filtered_roots: Vec<TagTreeNode> = if app.search_query.is_empty() {
            app.tag_tree_roots.clone()
        } else {
            app.tag_tree_roots.iter().filter_map(|node| {
                filter_tag_node(node, &app.search_query, app.search_mode)
            }).collect()
        };
        create_left_panel_tag_tree_browser_from_nodes(
            &filtered_roots,
            tree_browser_style,
            flat_button_style,
            app.left_panel_sort_mode,
        )
    },
};
```

This may require refactoring the existing `create_left_panel_file_tree_browser`
and `create_left_panel_tag_tree_browser` to accept a pre-filtered slice instead
of reading from `app.root_nodes` / `app.tag_tree_roots` directly.

**Remove buttons with filtered roots:** `create_left_panel_file_tree_browser`
uses `app.top_dirs.get(i)` indexed by `i` to build remove buttons. When
filtering removes a root node, the indices no longer align. The refactored
`create_left_panel_file_tree_browser_from_nodes` should accept a paired
`Vec<(Option<FileNode>, PathBuf)>` so each node carries its directory's
remove-button data, or disable remove buttons when `!search_query.is_empty()`.

**Cloning cost:** Filtering clones the tree each render. For large trees this
is a concern; see Section 6. For the initial implementation this is acceptable
because the alternative (mutating the model) would break the pure-Elm flow and
introduce complexity around persistence and expansion state.

---

## 4. UI Changes

### 4.1 Search box row (in `left_panel.rs`)

Add a new function:

```rust
fn create_search_row(app: &FileTreeApp, menu_style: MenuStyle) -> Element<'_, Message>;
```

This row contains:

- A `iced::widget::TextInput` for typing the search query
- A small button cycling the `TextSearchMode` label (e.g. "🔍 All", "🔍 Path",
  "🔍 Filename", "🔍 Tags")

The row is inserted between `left_panel_menu_row_2` and `tree_browser` in
`create_left_panel()`.

#### Text input styling

```rust
text_input::<Message, iced::Theme, iced::Renderer>(
    "Search...",                        // placeholder
    &app.search_query,                  // current value
    Message::SearchQueryChanged,        // on input
)
```

Use `menu_style.text_size` for font size and a dark background with light text.

#### Mode toggle button

```rust
let mode_label = match app.search_mode {
    TextSearchMode::All => "🔍 All",
    TextSearchMode::DirectoryPath => "🔍 Path",
    TextSearchMode::TrackFilename => "🔍 File",
    TextSearchMode::Creator => "🔍 Artist",
    TextSearchMode::Album => "🔍 Album",
    TextSearchMode::Title => "🔍 Title",
    TextSearchMode::Genre => "🔍 Genre",
};
button(text(mode_label).size(menu_style.text_size))
    .on_press(Message::ToggleSearchMode)
```

### 4.2 Insertion point in `create_left_panel()`

The search row must be placed inside the `if app.left_panel_expanded` branch,
not after it, so it is hidden when the panel collapses.

Change the left panel assembly from:

```rust
column![
    left_panel_menu_row_1,
    Space::with_height(10),
    left_panel_menu_row_2,
    Space::with_height(10),
    tree_browser,
]
```

to:

```rust
column![
    left_panel_menu_row_1,
    Space::with_height(10),
    left_panel_menu_row_2,
    Space::with_height(10),
    create_search_row(app, menu_style),
    Space::with_height(5),
    tree_browser,
]
```

---

## 5. Implementation Order (Commit Plan)

### Commit 1 — Add types, model fields, and messages

- Add `TextSearchMode` enum to `state.rs`
- Add `search_query: String` and `search_mode: TextSearchMode` to `FileTreeApp`
- Initialise both in `FileTreeApp::new()`
- Add `SearchQueryChanged(String)` and `ToggleSearchMode` to `Message` enum
- Export `TextSearchMode` from `gui/mod.rs`
- **Test:** Unit test that a freshly created `FileTreeApp` has `search_query == ""`
  and `search_mode == TextSearchMode::All`.

### Commit 2 — Wire update handlers

- Add both message arms to `update()` in `update.rs`
- **Test:** Unit test that sending `SearchQueryChanged("test")` sets
  `app.search_query == "test"`. Unit test that `ToggleSearchMode` cycles
  through all seven modes correctly (including wrapping back to `All`).

### Commit 3 — Add the search bar UI

- Add `create_search_row()` to `left_panel.rs`
- Insert it into `create_left_panel()` between row 2 and the tree browser
- **Test:** Smoke test that the view renders without panicking when a query
  is set. Note: verifying the mode button label against the current
  `search_mode` requires rendering the iced widget tree — the existing test
  suite uses state assertions on `FileTreeApp`, not widget-tree introspection,
  so this check is best performed by asserting that `create_search_row` does
  not panic and that the mode cycling (Commit 2) produces the correct label
  string.

### Commit 4 — Implement file tree filtering

- Add `filter_file_node()` to `left_panel.rs` (or a new helper module)
- Refactor `create_left_panel_file_tree_browser` to accept filtered nodes
- Wire filtering into `create_left_panel()`
- **Test:** Test filtering a small `FileNode` tree with each mode:
  - Query matches directory name in `DirectoryPath` mode
  - Query matches filename in `TrackFilename` mode
  - Query matches metadata in each individual tag mode (`Creator`, `Album`,
    `Title`, `Genre`) — test the extracted `file_matches_metadata_mode`
    helper directly with hardcoded `MediaMetadata` values
  - Empty query returns all nodes
  - Query that matches nothing returns no tree elements
- **Property-based test:** `filter_file_node(tree, "", _, _) == Some(tree)`
  (identity on empty query). `filter_file_node(tree, q, mode, _)` preserves
  tree structure to matching leaves (determinism invariant).

### Commit 5 — Implement tag tree filtering

- Add `filter_tag_node()` to `left_panel.rs`
- Refactor `create_left_panel_tag_tree_browser` to accept filtered nodes
- Wire filtering into `create_left_panel()`
- **Test:** Test filtering a small `TagTreeNode` tree with each mode:
  - Query matches label in each individual tag mode (`Genre`, `Creator`,
    `Album`, `Title`) — tag tree labels correspond to these fields
  - Query matches path in `DirectoryPath` mode
  - Query matches filename in `TrackFilename` mode
  - Query matches all fields in `All` mode
  - Empty query returns all nodes
  - Non-matching query produces empty tree
- **Property-based test:** `filter_tag_node(tree, "", _) == Some(tree)` on
  empty query. Filtering preserves the original tree ordering.

### Commit 6 — Clippy, docs, and final polish

- Run `cargo clippy` and fix any warnings
- Run `cargo test` to confirm all tests pass
- Add/update module-level docstrings for new items
- Update `AGENTS.md`'s `sorted_right_panel_files` note if search fields
  overlap with metadata
- Final manual review

---

## 6. Future Work / Considerations

### 6.1 Performance

- **Metadata modes (`Creator`, `Album`, `Title`, `Genre`) on file trees**
  call `extract_media_metadata` for every file node on every render.
  Mitigations:
  - Memoise metadata in `FileTreeApp` (`HashMap<PathBuf, MediaMetadata>`)
    populated lazily on first search and invalidated when extensions change
    **(implemented in v1 — see Section 3.1)**.
  - Debounce the search input in `update()` — only start filtering after
    the user stops typing for ~200ms (not in v1).
  - Clip tree rendering depth when a query is active (only expand matching
    branches, collapse non-matching ones) (not in v1).

### 6.2 UX refinements

- **Search history / recent searches** — store last N queries in `sled`.
- **Regex search** — prefix `/pattern/` for regex matching.
- **Count badge** — show "(N matches)" next to the search input.
- **Esc key** — clear the search query when Escape is pressed.

### 6.3 iced API notes (iced 0.13)

- `TextInput` in iced 0.13 uses `on_input` for change events. The
  `SearchQueryChanged(String)` message matches this signature directly.
  The third positional argument to `TextInput::new` / the `text_input`
  free function IS the on-input handler — do NOT also chain `.on_input()`
  because it is redundant.
- The widget's `style` function accepts a theme, so the dark-background
  styling follows the same pattern as the existing `flat_button_style`.

### 6.4 Data staging concern

Filtering logic lives in the view layer (`left_panel.rs`), which mixes
data transformation with presentation. This is acceptable for an Elm-
architecture UI where the view is a pure function of state, but the
metadata extraction inside `filter_file_node` leaks an expensive I/O
operation into the render path. The metadata cache (Section 3.1)
mitigates this by ensuring each file's metadata is extracted at most
once between cache invalidations.
