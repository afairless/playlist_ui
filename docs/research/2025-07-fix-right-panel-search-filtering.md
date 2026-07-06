# Fix Right Panel Search Filtering — Remove Search from Right Panel

**Date:** 2026-07-06
**Status:** Needs Implementation

## 1. Bug Description

The right panel is supposed to show the **cumulative playlist** — every file
the user has added, regardless of the search query in the left panel. Instead,
when a search query is active, the right panel only shows files whose paths
happen to match the search term. Files added before the search was typed
disappear; files added during search may or may not appear depending on
whether their paths are in the tantivy match set.

### Reproducible scenario

1. No search → add file A to playlist → A appears in right panel ✓
2. Type a search term → file A **disappears** from right panel ✗
3. While search is active → add file B (right-click) → B appears ✓
4. Clear search term → both A and B appear ✓

Step 2 is the core bug: entering a search query should not filter the right
panel at all. The search box is for the left panel only.

## 2. Root Cause

The right panel view (`create_right_panel` in `right_panel.rs`) branches on
whether a search query is active:

```rust
// right_panel.rs, create_right_panel(), lines 270-274
let displayed_files: Vec<RightPanelFile> = if app.search_query.is_empty() {
    app.sorted_right_panel_files()        // all files, sorted
} else {
    app.filtered_right_panel_files.clone() // subset that match search
};
```

`filtered_right_panel_files` is a `right_panel_files` subset filtered against
the tantivy search match set, populated by `perform_search()` in `state.rs`:

```rust
// state.rs, perform_search(), lines 334-338
self.filtered_right_panel_files = self
    .right_panel_files
    .iter()
    .filter(|f| matches.contains(&f.path))
    .cloned()
    .collect();
```

There are also related issues in the export/play path:

```rust
// update.rs, displayed_right_panel_files(), lines 69-73
let files = if app.search_query.is_empty() {
    app.right_panel_files.clone()
} else {
    app.filtered_right_panel_files.clone() // same bug for Export/Play
};
```

A prior fix (see `docs/research/2025-07-right-panel-search-display-bug.md`)
added `refilter_right_panel_files()` to keep `filtered_right_panel_files`
in sync when files are added/removed during search. That approach kept the
design where the right panel is filtered by search. This plan takes the
corrective approach: **the right panel should never be filtered by search
at all**.

## 3. Proposed Fix

### Guiding principle

The right panel is the cumulative playlist. The search box filters the **left**
panel only — it narrows what the user browses to find files to add. Once
added to the playlist, files should remain visible regardless of the search
state.

### Changes needed

| # | File | Change |
|---|------|--------|
| 1 | `src/gui/right_panel.rs` | Remove search branching in `create_right_panel`; always use `sorted_right_panel_files()` |
| 2 | `src/gui/update.rs` | Remove search branching in `displayed_right_panel_files`; always use `right_panel_files` |
| 3 | `src/gui/state.rs` | Remove `filtered_right_panel_files` filtering from `perform_search()` |
| 4 | `src/gui/update.rs` | Remove all `filtered_right_panel_files` writes: `refilter_right_panel_files()` calls (6 handlers) and `Vec::new()` resets (4 handlers) |
| 5 | `src/gui/state.rs` | Remove `refilter_right_panel_files()` method, `filtered_right_panel_files` field, and `new()` initialiser |
| 6 | `src/gui/update.rs` | Remove tests that exercise `filtered_right_panel_files` and add tests that verify right panel ignores search |

### Step 1: Fix `create_right_panel` in `right_panel.rs`

Replace the search branching with a direct call to `sorted_right_panel_files()`:

```rust
// Before:
let displayed_files: Vec<RightPanelFile> = if app.search_query.is_empty() {
    app.sorted_right_panel_files()
} else {
    app.filtered_right_panel_files.clone()
};

// After:
let displayed_files: Vec<RightPanelFile> = app.sorted_right_panel_files();
```

### Step 2: Fix `displayed_right_panel_files` in `update.rs`

Replace the search branching with a direct clone of `right_panel_files`:

```rust
// Before:
fn displayed_right_panel_files(app: &FileTreeApp) -> Vec<RightPanelFile> {
    let files = if app.search_query.is_empty() {
        app.right_panel_files.clone()
    } else {
        app.filtered_right_panel_files.clone()
    };
    let mut files = files;
    // ... sorting ...
    files
}

// After:
fn displayed_right_panel_files(app: &FileTreeApp) -> Vec<RightPanelFile> {
    let mut files = app.right_panel_files.clone();
    // ... sorting (unchanged) ...
    files
}
```

### Step 3: Remove `filtered_right_panel_files` population from `perform_search()`

In `state.rs`, remove the block that filters `filtered_right_panel_files`.
The rest of `perform_search()` (building `last_search_matches` and
populating `filtered_root_nodes`/`filtered_tag_tree_roots`) remains unchanged
— it is still needed for left-panel filtering:

```rust
// Remove these lines from perform_search():
self.filtered_right_panel_files = self
    .right_panel_files
    .iter()
    .filter(|f| matches.contains(&f.path))
    .cloned()
    .collect();
```

### Step 4: Remove all `filtered_right_panel_files` writes from `update.rs`

Remove `app.refilter_right_panel_files();` calls from these six message
handler arms:

- `AddToRightPanel`
- `AddDirectoryToRightPanel`
- `AddTagNodeToRightPanel`
- `RemoveFromRightPanel`
- `RemoveDirectoryFromRightPanel`
- `ClearRightPanel`

Remove `app.filtered_right_panel_files = Vec::new();` resets from these
four handler arms (they exist alongside the `filtered_root_nodes` and
`filtered_tag_tree_roots` resets in the no-search branches):

- `SearchCleared`
- `SearchQueryChanged` (empty-query branch)
- `RemoveTopDir` (no-search branch)
- `DirectoryAdded` (both the initial-state block and the no-search branch)

### Step 5: Remove dead code from `state.rs`

Remove the following from `FileTreeApp` and its `impl` block:

- **Field**: `pub filtered_right_panel_files: Vec<RightPanelFile>`
- **Method**: `pub(crate) fn refilter_right_panel_files(&mut self) { ... }`
- Remove from `new()`: `filtered_right_panel_files: Vec::new()`

All references to `filtered_right_panel_files` in `update.rs` were already
cleaned up in Step 4, so the field can be removed without any compile errors.

### Step 6: Update tests

Remove tests that exercise `filtered_right_panel_files`:

- `test_toggle_search_mode_changes_filtered_right_panel` (~line 1135)
- `test_refilter_right_panel_noop_when_no_search` (~line 1210)
- `test_refilter_right_panel_noop_when_no_matches` (~line 1235)
- `test_add_to_right_panel_updates_filtered_during_search` (~line 1255)
- `test_add_file_not_in_matches_excluded_from_filtered` (~line 1270)
- `test_remove_from_right_panel_updates_filtered_during_search` (~line 1285)
- `test_clear_right_panel_updates_filtered_during_search` (~line 1320)
- Helper `app_with_search_and_matches` (~line 1200)

Also remove `filtered_right_panel_files` from any test state assertions
that reference it (e.g. `assert!(deserialized.filtered_right_panel_files.is_empty())`).

Add new tests that verify right panel display ignores search state:

```rust
/// File added before search remains visible after search activated
#[test]
fn test_right_panel_shows_all_files_regardless_of_search() {
    let mut app = FileTreeApp::new(
        vec![],
        &["mp3"],
        PathBuf::from("/tmp/test.json"),
        None,
    );

    // Add file A while no search is active
    let _ = update(
        &mut app,
        Message::AddToRightPanel(PathBuf::from("/music/song_a.mp3")),
    );
    assert_eq!(app.right_panel_files.len(), 1);

    // Activate a search — this should NOT affect right_panel_files
    let _ = update(
        &mut app,
        Message::SearchQueryChanged("zzz_nonexistent".to_string()),
    );

    // The file should still be in the playlist
    assert_eq!(
        app.right_panel_files.len(),
        1,
        "playlist should retain all files despite search"
    );

    // sorted_right_panel_files() should return the file
    assert_eq!(app.sorted_right_panel_files().len(), 1);
}

/// displayed_right_panel_files always returns all files regardless of
/// search state. Covers both the pre-search state (last_search_matches
/// is None) and the active-search state (last_search_matches exists but
/// contains no matches for the playlist file).
#[test]
fn test_displayed_right_panel_files_ignores_search() {
    let mut app = FileTreeApp::new(
        vec![],
        &["mp3"],
        PathBuf::from("/tmp/test.json"),
        None,
    );

    let _ = update(
        &mut app,
        Message::AddToRightPanel(PathBuf::from("/music/song.mp3")),
    );
    assert_eq!(app.right_panel_files.len(), 1);

    // Pre-search state: last_search_matches is None (no tantivy search
    // has run yet). displayed_right_panel_files should still return all
    // files.
    app.search_query = "something".to_string();
    assert!(
        app.last_search_matches.is_none(),
        "pre-search state: no matches cached"
    );
    let displayed = displayed_right_panel_files(&app);
    assert_eq!(
        displayed.len(),
        1,
        "displayed files should ignore pre-search state"
    );

    // Active-search state: last_search_matches exists but is empty.
    app.last_search_matches = Some(HashSet::new());
    let displayed = displayed_right_panel_files(&app);
    assert_eq!(
        displayed.len(),
        1,
        "displayed files should ignore active search state"
    );
}
```

## 4. Impact and Risk Assessment

- **Risk**: Low. This removes broken filtering, making the right panel behave
  as users expect: always showing the full cumulative playlist.
- **Performance**: Slight improvement. `perform_search()` no longer iterates
  `right_panel_files` for filtering. The `refilter_right_panel_files` calls
  in six handlers are removed (saves O(n) per add/remove).
- **Backward compatibility**: Breaking change in behavior only — users who
  relied on the search box filtering the right panel will now see the full
  playlist instead. This matches the stated design intent. No persisted-state
  impact: `filtered_right_panel_files` is `#[serde(skip)]` and is never
  serialized, so removing it won't break deserialization of
  `~/.playlist_ui_top_dirs.json`.
- **Edge cases**:
  - Shuffled state: `sorted_right_panel_files()` skips sorting when
    `right_panel_shuffled` is true, returning files in shuffled order.
    This is correct — the shuffled order should persist regardless of search.
  - Export/Play: `displayed_right_panel_files` will now export the full
    playlist even during search, consistent with the right panel display.
  - Empty playlist: No change — returns empty vector as before.

## 5. Comparison with Prior Plan

The prior plan (`2025-07-right-panel-search-display-bug.md`) took the approach
of keeping `filtered_right_panel_files` in sync with `right_panel_files` by
calling `refilter_right_panel_files()` in every add/remove handler. That plan
was partially implemented (the helper and calls exist) but doesn't address
the fundamental issue: files already in the playlist disappear when search is
activated because they don't match the search term.

This plan removes search filtering from the right panel entirely, which
matches the stated design goal: "The purpose of the right panel is to see
the cumulative music tracks that have been added to the playlist, regardless
of what is currently appearing in the left panel."

## 6. Implementation Order

Each step compiles and passes all tests independently. Steps are ordered so
that all references to `filtered_right_panel_files` are removed before the
field itself is deleted. Steps 1-2 are independent of each other and can be
swapped.

1. Fix `create_right_panel` in `right_panel.rs` (core bug)
2. Fix `displayed_right_panel_files` in `update.rs` (export/play path)
3. Remove `filtered_right_panel_files` filtering from `perform_search()` in `state.rs`
4. Remove all `filtered_right_panel_files` writes from `update.rs`:
   - `refilter_right_panel_files()` calls (6 handlers)
   - `Vec::new()` resets in `SearchCleared`, `SearchQueryChanged`, `RemoveTopDir`, `DirectoryAdded` (4 handlers)
5. Remove `refilter_right_panel_files()` method and `filtered_right_panel_files` field from `state.rs` (including the `new()` initialiser)
6. Remove old tests, add new tests
7. Run `cargo test && cargo clippy && cargo fmt`
