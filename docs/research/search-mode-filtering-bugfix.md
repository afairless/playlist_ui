# Search Mode Filtering Bugfix — Mode Has No Effect on Results

## Overview

The text search box in the left panel has seven configurable modes (All,
DirectoryPath, TrackFilename, Creator, Album, Title, Genre). Cycling through
these modes has no visible effect on search results because two independent
bugs prevent the mode from being applied to the two main data displays: the
**right panel** (playlist) is never filtered by search at all, and the **tag
tree** (genre/creator hierarchy in the left panel) filters by label only,
ignoring both the mode and file-level metadata.

---

## Root-Cause Analysis

### Bug 1 — Right Panel Is Never Filtered by Search

**Location:** `src/gui/right_panel.rs:132`

```rust
pub(crate) fn create_right_panel(
    app: &FileTreeApp, ...
) -> Element<'_, Message> {
    let displayed_files = app.sorted_right_panel_files();
    //                               ^^^^^^^^^^^^^^^^^^^^^^^^
    //  Always returns ALL right_panel_files, regardless of
    //  search_query or search_mode.
```

The `create_right_panel` function calls `app.sorted_right_panel_files()` which
returns every file in `app.right_panel_files` unchanged (only sorts). The
`Message::SearchQueryChanged` handler in `update.rs` only recomputes
`filtered_root_nodes` and `filtered_tag_tree_roots` — it never touches the
right panel.

**Symptom:** The user types "little" in the search box with mode set to
"Genre". The left-panel file tree correctly hides non-matching files (those
whose genre tag doesn't contain "little"). But the right-panel playlist
continues to display all previously added tracks, including the one with
"little" in its title. Cycling through all seven search modes produces zero
change in what the user sees — the right panel dominates the visual space,
making the search feel completely broken.

### Bug 2 — `filter_tag_node` Ignores the Search Mode Entirely

**Location:** `src/gui/left_panel.rs:444` and `src/gui/update.rs:52`

```rust
// left_panel.rs — no mode parameter
pub(crate) fn filter_tag_node(
    node: &TagTreeNode,
    query: &str,                         // ← mode is absent
) -> Option<TagTreeNode> {
    let label_matches =
        node.label.to_ascii_lowercase().contains(&query.to_ascii_lowercase());
    //                              ^^^^^^^^^^
    //  Only checks the label against the query string.
    //  Does not inspect file_paths, does not consult the search mode.
}

// update.rs — call site omits mode
fn recompute_filtered_tag_nodes(app: &FileTreeApp) -> Vec<TagTreeNode> {
    ...
    .filter_map(|node| filter_tag_node(node, &app.search_query))
    //                                                ^^^^^^^^
    //  Only passes the query string, not app.search_mode.
```

The function only checks whether a node's **label** contains the query string
(case-insensitive). It never checks `file_paths` against the active search
mode. This means:

- Switching from "Genre" to "Creator" to "Album" to "Title" mode produces
  identical filtering — the same label-only matching each time.
- A query like "little" would match a genre/creator/album node only if
  that node's *label* (not the metadata of its files) contains "little".
- The `recompute_filtered_tag_nodes` call site doesn't pass `app.search_mode`
  at all, so even if `filter_tag_node` accepted a mode, it wouldn't receive it.

### Why These Two Bugs Together Explain the User's Report

1. User is in **Genre tag-tree view** (`LeftPanelSelectMode::GenreTag`),
   types "little" with search mode `TextSearchMode::Genre`.
2. `recompute_filtered_tag_nodes` runs → `filter_tag_node` only checks labels
   against "little". No genre label contains "little" → filtered tag tree is
   empty. The left panel shows nothing useful.
3. User looks at the **right panel** (playlist), which occupies most of the
   window. It still shows every track they've added, including the one with
   "little" in its title — because the right panel is **never** filtered.
4. User cycles through all seven search modes → the right panel never changes
   → all modes appear to produce identical results.

---

## Why Existing Tests Didn't Catch This

### Missing: Right-panel search filtering tests

No test anywhere verifies that the right panel's displayed files are affected
by the search query or mode. The `create_right_panel` test in `view.rs` only
checks that the function doesn't panic — it never asserts which files appear.

### Missing: Metadata-mode `filter_file_node` tests

All existing `filter_file_node` tests (in `left_panel.rs`) exercise only:

- Empty query (returns `Some`)
- `DirectoryPath` mode (path matching)
- `TrackFilename` mode (filename matching)
- `All` mode (path / filename matching)

There are **zero tests** for `TextSearchMode::Creator`, `TextSearchMode::Album`,
`TextSearchMode::Title`, or `TextSearchMode::Genre`.  The helper function
`file_matches_mode` has no dedicated tests either.

Further, existing test fixtures create `FileNode` values with `test_file()`
which only sets the name and path — no metadata fields are populated, and the
test paths don't exist on disk (so `extract_media_metadata` returns all-
`None`).  This means metadata-mode tests would *pass* even if the logic were
wrong (they'd always return `None` because metadata is unavailable).

### Missing: `filter_tag_node` mode-awareness tests

`filter_tag_node` tests only check label matching — they never test:

- Metadata field matching against `file_paths`
- Mode-aware filtering (e.g., "Genre" mode should check genre labels, "Album"
  mode should check album metadata against file paths)
- `DirectoryPath` or `TrackFilename` mode matching against tag tree paths

### Missing: `ToggleSearchMode` integration test

The existing test `test_toggle_search_mode_cycles_all_modes` only asserts that
`app.search_mode` cycles through the enum values. It never verifies that the
*filtered output* (`app.filtered_root_nodes` or
`app.filtered_tag_tree_roots`) changes when the mode changes.

---

## Proposed Fix

### Shared Prerequisite — Extract `file_field_matches` to a Shared Utility

The private `field_matches` function in `left_panel.rs` performs a
case-insensitive substring check on an `Option<String>` field. Both the right
panel (which has pre-extracted `RightPanelFile` fields) and the left panel
(tag tree filtering) need this comparison. Extract it as a public utility:

**File:** `src/utils.rs`

```rust
/// Checks whether an optional string field contains the given query
/// (case-insensitive). Returns `false` when the field is `None`.
pub(crate) fn file_field_matches(
    value: &Option<String>,
    query: &str,
) -> bool {
    value
        .as_deref()
        .map(|v| v.to_ascii_lowercase().contains(&query.to_ascii_lowercase()))
        .unwrap_or(false)
}
```

Then import and use it in `left_panel.rs` (replacing the private `field_matches`)
and in `right_panel.rs` and `update.rs`.

### Part A — Add `filtered_right_panel_files` Model Field and Filter in `update()`

**Do not filter inside the view function.** In the Elm architecture, the view
(`create_right_panel`) should render state, not compute it. Instead, add a
`filtered_right_panel_files` field to `FileTreeApp` and update it in the
`SearchQueryChanged` and `ToggleSearchMode` handlers, mirroring how
`filtered_root_nodes` and `filtered_tag_tree_roots` are already handled.

**File:** `src/gui/state.rs`

Add to `FileTreeApp`:

```rust
#[serde(skip)]
pub filtered_right_panel_files: Vec<RightPanelFile>,
```

Initialize it from `right_panel_files` in `FileTreeApp::new()`:

```rust
filtered_right_panel_files: Vec::new(),
```

**File:** `src/gui/update.rs`

Add a helper:

```rust
fn recompute_filtered_right_panel_files(
    app: &FileTreeApp,
) -> Vec<RightPanelFile> {
    if app.search_query.is_empty() {
        return app.right_panel_files.clone();
    }
    let query = &app.search_query;
    app.right_panel_files
        .iter()
        .filter(|f| match app.search_mode {
            TextSearchMode::All => {
                file_field_matches(&f.creator, query)
                    || file_field_matches(&f.album, query)
                    || file_field_matches(&f.title, query)
                    || file_field_matches(&f.genre, query)
            },
            TextSearchMode::Creator => file_field_matches(&f.creator, query),
            TextSearchMode::Album => file_field_matches(&f.album, query),
            TextSearchMode::Title => file_field_matches(&f.title, query),
            TextSearchMode::Genre => file_field_matches(&f.genre, query),
            TextSearchMode::DirectoryPath => f
                .path
                .to_string_lossy()
                .to_ascii_lowercase()
                .contains(&query.to_ascii_lowercase()),
            TextSearchMode::TrackFilename => f
                .path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_ascii_lowercase()
                .contains(&query.to_ascii_lowercase()),
        })
        .cloned()
        .collect()
}
```

Note: `file_field_matches` already lowercases its query internally, so pass the
raw `&app.search_query` — do not pre-lowercase.

Call this from both the `SearchQueryChanged` and `ToggleSearchMode` handlers:

```rust
Message::SearchQueryChanged(query) => {
    app.search_query = query;
    app.filtered_root_nodes = recompute_filtered_nodes(app);
    app.filtered_tag_tree_roots = recompute_filtered_tag_nodes(app);
    app.filtered_right_panel_files =
        recompute_filtered_right_panel_files(app);  // ← new
    Task::none()
},
Message::ToggleSearchMode => {
    app.search_mode = match app.search_mode { /* ... */ };
    app.filtered_root_nodes = recompute_filtered_nodes(app);
    app.filtered_tag_tree_roots = recompute_filtered_tag_nodes(app);
    app.filtered_right_panel_files =
        recompute_filtered_right_panel_files(app);  // ← new
    Task::none()
},
```

**File:** `src/gui/right_panel.rs`

Replace:

```rust
let displayed_files = app.sorted_right_panel_files();
```

With:

```rust
let displayed_files = &app.filtered_right_panel_files;
```

The view now reads pre-filtered state — no filtering logic in the view

### Part B — Pass the Search Mode Into `filter_tag_node` (Label-Based Approach)

**File:** `src/gui/left_panel.rs`

Change the signature:

```rust
pub(crate) fn filter_tag_node(
    node: &TagTreeNode,
    query: &str,
    mode: TextSearchMode,       // new parameter
) -> Option<TagTreeNode> {
```

**Approach:** The tag tree is *already* organised by metadata (Genre →
Artist → Album → Track).  Its node labels *are* the metadata values. Do
NOT call `extract_media_metadata` here — that would trigger slow disk I/O
on every keystroke for every file in every tag node.

Instead, use a label-based strategy:

- **Metadata modes** (`Creator`, `Album`, `Title`, `Genre`, `All`): match
  the query against node labels. In metadata mode, the label is the value
  being searched for (e.g., in Genre mode, the genre node label is the
  genre name). This is O(n) in the number of tag nodes and requires zero
  disk I/O.
- **`DirectoryPath` mode**: match the query against each `file_path` in
  the subtree via substring check on the path string (no metadata I/O).
- **`TrackFilename` mode**: match the query against the filename portion
  of each `file_path` (no metadata I/O).

Updated matching logic:

```rust
let query_lower = query.to_ascii_lowercase();
let label_matches =
    node.label.to_ascii_lowercase().contains(&query_lower);

// For file-path-based modes, check file_paths directly (no metadata).
let path_matches = !node.file_paths.is_empty()
    && match mode {
        TextSearchMode::DirectoryPath => node.file_paths.iter().any(|p| {
            p.to_string_lossy()
                .to_ascii_lowercase()
                .contains(&query_lower)
        }),
        TextSearchMode::TrackFilename => node.file_paths.iter().any(|p| {
            p.file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_ascii_lowercase()
                .contains(&query_lower)
        }),
        // Metadata modes: label matching is sufficient — the node's label
        // already represents the metadata category. No disk I/O needed.
        TextSearchMode::Creator
        | TextSearchMode::Album
        | TextSearchMode::Title
        | TextSearchMode::Genre
        | TextSearchMode::All => false,
    };

if node.children.is_empty() {
    // Leaf node (track)
    let matches = match mode {
        // For metadata modes, label is the relevant field value
        TextSearchMode::Creator
        | TextSearchMode::Album
        | TextSearchMode::Title
        | TextSearchMode::Genre => label_matches,
        // For broad modes, check label or file path
        TextSearchMode::All => label_matches || path_matches,
        // For path/filename modes, check file_paths or label
        TextSearchMode::DirectoryPath | TextSearchMode::TrackFilename => {
            label_matches || path_matches
        },
    };
    return if matches { Some(node.clone()) } else { None };
}

// Non-leaf node
if label_matches {
    // Label matches — keep the node with all children
    Some(node.clone())
} else {
    // Prune children, keep only matching subtrees
    let filtered_children: Vec<TagTreeNode> = node
        .children
        .iter()
        .filter_map(|child| filter_tag_node(child, query, mode))
        .collect();
    if filtered_children.is_empty() {
        None
    } else {
        let mut cloned = node.clone();
        cloned.children = filtered_children;
        Some(cloned)
    }
}
```

### Part C — Update Call Sites in `update.rs`

**File:** `src/gui/update.rs`

Import `file_field_matches` from the shared utility:

```rust
use crate::utils::file_field_matches;
```

Update `recompute_filtered_tag_nodes` to pass the search mode:

```rust
fn recompute_filtered_tag_nodes(app: &FileTreeApp) -> Vec<TagTreeNode> {
    if app.search_query.is_empty() {
        app.tag_tree_roots.clone()
    } else {
        app.tag_tree_roots
            .iter()
            .filter_map(|node| filter_tag_node(
                node,
                &app.search_query,
                app.search_mode,    // now passed
            ))
            .collect()
    }
}
```

Also update `SearchQueryChanged` and `ToggleSearchMode` handlers to recompute
`filtered_right_panel_files` (see Part A).

### Part D — Add Comprehensive Unit Tests

#### D.1 Right-panel filtering tests (new test module)

Add tests in `src/gui/right_panel.rs` (or in `update.rs` if the filtering
helper is added as a `FileTreeApp` method):

| Test | Description |
|------|-------------|
| `test_right_panel_filter_empty_query` | Empty query returns all files |
| `test_right_panel_filter_genre_mode` | Genre mode keeps files matching genre field, excludes others |
| `test_right_panel_filter_title_mode` | Title mode keeps files matching title, excludes files matching other fields only |
| `test_right_panel_filter_album_mode` | Album mode |
| `test_right_panel_filter_creator_mode` | Creator mode |
| `test_right_panel_filter_path_mode` | DirectoryPath mode checks path string |
| `test_right_panel_filter_filename_mode` | TrackFilename mode checks filename |
| `test_right_panel_filter_all_mode` | All mode matches any metadata field |
| `test_right_panel_filter_no_match` | Non-matching query returns empty |
| `test_right_panel_filter_case_insensitive` | Query is case-insensitive |
| `test_right_panel_filter_metadata_none_fields` | Files with None metadata are excluded in metadata modes |

#### D.2 `filter_file_node` metadata-mode tests (in `left_panel.rs`)

| Test | Description |
|------|-------------|
| `test_filter_genre_mode_matches_genre_metadata` | A file whose genre field matches the query is kept (use a pre-populated metadata path or mock) |
| `test_filter_creator_mode_matches_creator` | Creator mode |
| `test_filter_album_mode_matches_album` | Album mode |
| `test_filter_title_mode_matches_title` | Title mode |
| `test_filter_metadata_mode_excludes_path_match` | Genre mode does NOT match by path even if query appears in path |
| `test_filter_metadata_mode_excludes_filename_match` | Genre mode does NOT match by filename even if query appears in filename |
| `test_filter_metadata_mode_excludes_other_fields` | Genre mode does NOT match files whose title contains query (only genre) |

**Note on test infrastructure:** The existing `test_file()` helper creates
`FileNode` values pointing to paths that don't exist on disk, so
`extract_media_metadata` returns default metadata (all `None` fields). To test
metadata modes properly, either:

1. Refactor `filter_file_node` / `file_matches_mode` to accept an optional
   pre-extracted `MediaMetadata` (avoiding disk I/O in tests), or
2. Write integration tests with real audio files in a temp directory, or
3. Extract the `field_matches` comparison into a pure function that accepts
   an `&Option<String>` directly, test that, and accept that
   `file_matches_mode` is tested via integration tests.

Option 1 (injectable metadata) is recommended because it keeps tests fast and
deterministic. The `file_field_matches` utility is already extracted into
`src/utils.rs` (see Shared Prerequisite above). Import it in `left_panel.rs`
and replace the private `field_matches`:

```rust
use crate::utils::file_field_matches;
```

Then `file_matches_mode` becomes a thin dispatch:

```rust
fn file_matches_mode(path: &Path, mode: TextSearchMode, query: &str) -> bool {
    let meta = extract_media_metadata(path);
    match mode {
        TextSearchMode::Creator => file_field_matches(&meta.creator, query),
        TextSearchMode::Album   => file_field_matches(&meta.album, query),
        TextSearchMode::Title   => file_field_matches(&meta.title, query),
        TextSearchMode::Genre   => file_field_matches(&meta.genre, query),
        TextSearchMode::All     => { /* ... */ },
        _ => false,
    }
}
```

And the right-panel filtering helper can use `file_field_matches` directly
(with pre-extracted metadata from `RightPanelFile` fields), without needing
the full `extract_media_metadata` path. Unit tests for `file_field_matches`
cover the string-comparison logic cheaply.

#### D.3 `filter_tag_node` mode-awareness tests (in `left_panel.rs`)

| Test | Description |
|------|-------------|
| `test_filter_tag_node_all_mode_matches_label` | All mode keeps node whose label matches |
| `test_filter_tag_node_all_mode_matches_file_path` | All mode keeps node whose file's title/creator/album/genre matches |
| `test_filter_tag_node_label_matches_in_any_mode` | Label match is sufficient in every mode |
| `test_filter_tag_node_file_path_matches_in_genre_mode` | Genre mode keeps node when a file's genre matches |
| `test_filter_tag_node_file_path_matches_in_album_mode` | Album mode keeps node when a file's album matches |
| `test_filter_tag_node_file_path_matches_in_creator_mode` | Creator mode |
| `test_filter_tag_node_file_path_matches_in_title_mode` | Title mode |
| `test_filter_tag_node_file_path_matches_in_path_mode` | DirectoryPath mode checks file paths |
| `test_filter_tag_node_file_path_matches_in_filename_mode` | TrackFilename mode checks filenames |
| `test_filter_tag_node_all_mode_checks_all_fields` | All mode checks label AND all metadata fields |

#### D.4 Integration test: `ToggleSearchMode` changes filtered output

In `update.rs`:

| Test | Description |
|------|-------------|
| `test_toggle_search_mode_changes_filtered_root_nodes` | Set up `root_nodes` with metadata-bearing files, toggle mode, verify `filtered_root_nodes` differs |
| `test_toggle_search_mode_changes_filtered_tag_roots` | Set up `tag_tree_roots` with metadata-bearing files, toggle mode, verify `filtered_tag_tree_roots` differs |
| `test_toggle_search_mode_changes_filtered_right_panel` | Set up `right_panel_files` with metadata-bearing files, toggle between Title and Genre mode with matching query, verify `filtered_right_panel_files` differs |

#### D.5 `file_field_matches` unit tests (in `utils.rs`)

| Test | Description |
|------|-------------|
| `test_file_field_matches_empty_query` | Empty query returns `true` for any `Some` value (empty string is contained in every string) |
| `test_file_field_matches_none_value` | `None` value returns `false` for any non-empty query |
| `test_file_field_matches_case_insensitive` | Matching value returns `true` regardless of case in the field or query |
| `test_file_field_matches_no_match` | Non-matching value returns `false` |
| `test_file_field_matches_partial_match` | Substring match returns `true` |

#### D.6 Property-based tests

| Test | Description |
|------|-------------|
| `prop_filter_right_panel_empty_query` | For any set of `RightPanelFile`s, filtering with empty query returns all files |
| `prop_filter_tag_node_empty_query` | For any `TagTreeNode`, `filter_tag_node(node, "", _) == Some(node)` |
| `prop_filter_file_node_empty_query` | For any `FileNode`, `filter_file_node(node, "", _) == Some(node)` |
| `prop_filter_idempotent` | For any query, `filter_*(node, q, m)` has the same result as running it twice (idempotency) |

### Part E — Documentation

Every new public or `pub(crate)` function requires a docstring (see
`docs/ARCHITECTURE.md` and the project's docstring conventions in `AGENTS.md`):

- `file_field_matches` in `utils.rs` — docstring explaining case-insensitive
  substring matching and `None` handling.
- `recompute_filtered_right_panel_files` in `update.rs` — docstring with
  filter semantics and mode dispatch.
- Update existing docstrings in `left_panel.rs` (`filter_tag_node`,
  `filter_file_node`) if the signature or behaviour changes.
