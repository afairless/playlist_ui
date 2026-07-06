# Right Panel Display Bug — Files Not Shown While Search Query Is Active

**Date:** 2026-07-06
**Status:** Needs Implementation

## 1. Bug Description

When a text search query is active in the left-panel search box, files added to
the right-panel playlist (via right-click context menu) do **not** appear in the
right panel. The files are correctly added to the underlying data structure
(`right_panel_files`), but the view layer uses `filtered_right_panel_files`
during an active search, which is **not** updated when files are
added/removed. When the search is cleared, the view falls back to
`sorted_right_panel_files()` and the previously-added files suddenly appear.

### Two reproducible scenarios

1. **Add while searching:** Type a search term → right-click a file in the left
   panel → "Add to right panel" → file does **not** appear in the right panel.
   Clear the search → file appears.

2. **Add before searching, then add while searching:** No search term → add
   file A (appears in right panel). Type a search term → file A **disappears**
   from the right panel. While search is active → add file B via right-click →
   file B does **not** appear. Clear the search → both A and B appear.

## 2. Root Cause Analysis

### Data flow

The right panel view (`right_panel.rs:create_right_panel`) selects which files
to display based on whether a search query is active:

```rust
// right_panel.rs, create_right_panel()
let displayed_files: Vec<RightPanelFile> = if app.search_query.is_empty() {
    app.sorted_right_panel_files()      // ← uses right_panel_files directly
} else {
    app.filtered_right_panel_files.clone()  // ← uses the filtered copy
};
```

The field `filtered_right_panel_files` is populated exclusively by
`FileTreeApp::perform_search()` (defined in `state.rs`), which filters
`right_panel_files` against the tantivy search match set:

```rust
// state.rs, perform_search()
self.filtered_right_panel_files = self
    .right_panel_files
    .iter()
    .filter(|f| matches.contains(&f.path))
    .cloned()
    .collect();
```

### When `perform_search()` is called

`perform_search()` is triggered by these message handlers:

| Message                    | Calls `perform_search`? |
|---------------------------|------------------------|
| `SearchQueryChanged`      | ✅ (when non-empty)     |
| `SearchCleared`           | ❌ (clears state)       |
| `ToggleSearchMode`        | ✅ (when non-empty)     |
| `ToggleExtension`         | ✅ (when non-empty)     |
| `RemoveTopDir`            | ✅ (when non-empty)     |
| `DirectoryAdded`          | ✅ (when non-empty)     |
| `ToggleLeftPanelSelectMode`| ✅ (when non-empty)    |
| **`AddToRightPanel`**     | ❌                      |
| **`AddDirectoryToRightPanel`** | ❌                 |
| **`AddTagNodeToRightPanel`**   | ❌                 |
| **`RemoveFromRightPanel`**     | ❌                 |
| **`RemoveDirectoryFromRightPanel`** | ❌           |
| **`ClearRightPanel`**     | ❌                      |

The six handlers marked ❌ modify `right_panel_files` but never update
`filtered_right_panel_files`. When a search is active, the view reads the
stale `filtered_right_panel_files`, so added files are invisible and removed
files persist visually until the search is cleared.

### Why scenario 2 also hides pre-existing files

When a search query is first typed, `SearchQueryChanged` → `perform_search()`
rebuilds `filtered_right_panel_files` from the current `right_panel_files`
filtered by tantivy matches. Previously-added files that don't match the
search term are correctly excluded from `filtered_right_panel_files`. This
part is **expected behavior** — the search filters both panels. The bug is
only that subsequent adds/removes (via the handlers above) are not reflected
in `filtered_right_panel_files`.

## 3. Proposed Fix

### Strategy

When any message handler modifies `right_panel_files` **while a search query
is active**, we must update `filtered_right_panel_files` to stay in sync. We
should **not** call `perform_search()` because that re-executes the full
tantivy search (expensive and unnecessary — the left-panel filtered trees
haven't changed). Instead, we re-filter using the **cached**
`last_search_matches`.

### Step 1: Add a helper method to `FileTreeApp`

```rust
// In state.rs, on FileTreeApp impl block

/// Updates `filtered_right_panel_files` by re-filtering the current
/// `right_panel_files` against the cached search matches. This is a
/// no-op when no search query is active or no cached matches exist.
fn refilter_right_panel_files(&mut self) {
    if self.search_query.is_empty() {
        return;
    }
    if let Some(ref matches) = self.last_search_matches {
        self.filtered_right_panel_files = self
            .right_panel_files
            .iter()
            .filter(|f| matches.contains(&f.path))
            .cloned()
            .collect();
    }
}
```

### Step 2: Call `refilter_right_panel_files()` in the six affected handlers

In `update.rs`, immediately after the code that mutates `right_panel_files`
(or `right_panel_shuffled`), insert:

```rust
app.refilter_right_panel_files();
```

The six handlers to modify:

1. **`AddToRightPanel`** — after `app.right_panel_files.push(...)`
2. **`AddDirectoryToRightPanel`** — after the loop that pushes files
3. **`AddTagNodeToRightPanel`** — after the loop that pushes files
4. **`RemoveFromRightPanel`** — after `app.right_panel_files.retain(...)`
5. **`RemoveDirectoryFromRightPanel`** — after `app.right_panel_files.retain(...)`
6. **`ClearRightPanel`** — after `app.right_panel_files.clear()`

### Step 3: Remove dead code `recompute_filtered_right_panel_files`

In `update.rs`, remove the `#[allow(dead_code)]` function
`recompute_filtered_right_panel_files`. This function performed text-based
(not tantivy-based) filtering of right-panel files and was never called. The
new `refilter_right_panel_files()` helper on `FileTreeApp` supersedes it by
reusing the cached tantivy match set for consistency with `perform_search()`.

Also remove the `use crate::utils::file_field_matches;` import if it is no
longer referenced after removing the function (it may still be used by
`filter_file_node`).

### Step 4: Add unit tests

Add tests in the `#[cfg(test)] mod tests` block of `update.rs` that verify
`filtered_right_panel_files` stays in sync when right-panel files are modified
while a search is active. Follow the **None-One-Many** principle:

**None (no-op paths):**

```rust
#[test]
fn test_refilter_right_panel_noop_when_no_search() {
    // Adding/removing files with no search active should not populate
    // filtered_right_panel_files (it remains the view's signal to use
    // sorted_right_panel_files instead).
    let mut app = FileTreeApp::new(
        vec![],
        &["mp3"],
        PathBuf::from("/tmp/test.json"),
        None,
    );
    // No search query — filtered should stay empty
    let msg = Message::AddToRightPanel(PathBuf::from("/music/song.mp3"));
    let _ = update(&mut app, msg);
    assert_eq!(app.right_panel_files.len(), 1);
    assert!(
        app.filtered_right_panel_files.is_empty(),
        "filtered should be empty when no search is active"
    );
}

#[test]
fn test_refilter_right_panel_noop_when_no_matches() {
    // If last_search_matches is None (no prior search executed),
    // the refilter is a no-op.
    let mut app = FileTreeApp::new(
        vec![],
        &["mp3"],
        PathBuf::from("/tmp/test.json"),
        None,
    );
    app.search_query = "song".to_string();
    // last_search_matches is None — filtered should stay empty
    let msg = Message::AddToRightPanel(PathBuf::from("/music/song.mp3"));
    let _ = update(&mut app, msg);
    assert_eq!(app.right_panel_files.len(), 1);
    assert!(
        app.filtered_right_panel_files.is_empty(),
        "filtered should be empty when last_search_matches is None"
    );
}
```

**One (single add/remove with search active):**

```rust
/// Helper: build an app with search active and a pre-seeded match set.
/// The match set is manually seeded because a real tantivy index requires
/// actual files on disk; this unit test exercises the refilter logic
/// directly without filesystem dependencies.
fn app_with_search_and_matches(
    match_paths: &[&str],
) -> FileTreeApp {
    let mut app = FileTreeApp::new(
        vec![],
        &["mp3"],
        PathBuf::from("/tmp/test.json"),
        None,
    );
    app.search_query = "song".to_string();
    app.last_search_matches = Some(
        match_paths
            .iter()
            .map(|p| PathBuf::from(p))
            .collect(),
    );
    app
}

#[test]
fn test_add_to_right_panel_updates_filtered_during_search() {
    let mut app = app_with_search_and_matches(&["/music/song.mp3"]);
    let msg = Message::AddToRightPanel(PathBuf::from("/music/song.mp3"));
    let _ = update(&mut app, msg);

    assert_eq!(app.right_panel_files.len(), 1);
    assert_eq!(
        app.filtered_right_panel_files.len(),
        1,
        "filtered_right_panel_files should reflect the added file"
    );
}

#[test]
fn test_add_file_not_in_matches_excluded_from_filtered() {
    // If the added file's path is not in last_search_matches, it should
    // not appear in the filtered list.
    let mut app = app_with_search_and_matches(&["/music/other.mp3"]);
    let msg = Message::AddToRightPanel(PathBuf::from("/music/song.mp3"));
    let _ = update(&mut app, msg);

    assert_eq!(app.right_panel_files.len(), 1);
    assert!(
        app.filtered_right_panel_files.is_empty(),
        "file not in match set should be excluded from filtered"
    );
}

#[test]
fn test_remove_from_right_panel_updates_filtered_during_search() {
    // Pre-populate right_panel_files and filtered_right_panel_files,
    // then remove one and verify filtered is updated.
    let mut app = app_with_search_and_matches(&["/music/a.mp3", "/music/b.mp3"]);
    // Simulate files already added (bypass add handler to set up state)
    app.right_panel_files.push(RightPanelFile {
        path: PathBuf::from("/music/a.mp3"),
        creator: None, album: None, title: None, genre: None, duration_ms: None,
    });
    app.right_panel_files.push(RightPanelFile {
        path: PathBuf::from("/music/b.mp3"),
        creator: None, album: None, title: None, genre: None, duration_ms: None,
    });
    app.filtered_right_panel_files = app.right_panel_files.clone();

    let msg = Message::RemoveFromRightPanel(PathBuf::from("/music/a.mp3"));
    let _ = update(&mut app, msg);

    assert_eq!(app.right_panel_files.len(), 1);
    assert_eq!(
        app.filtered_right_panel_files.len(),
        1,
        "filtered should reflect removal"
    );
}
```

**Many (bulk operations):**

```rust
#[test]
fn test_clear_right_panel_updates_filtered_during_search() {
    let mut app = app_with_search_and_matches(&["/music/a.mp3"]);
    app.right_panel_files.push(RightPanelFile {
        path: PathBuf::from("/music/a.mp3"),
        creator: None, album: None, title: None, genre: None, duration_ms: None,
    });
    app.filtered_right_panel_files = app.right_panel_files.clone();

    let msg = Message::ClearRightPanel;
    let _ = update(&mut app, msg);

    assert!(app.right_panel_files.is_empty());
    assert!(
        app.filtered_right_panel_files.is_empty(),
        "filtered should be empty after clear"
    );
}
```

## 4. Impact and Risk Assessment

- **Risk:** Low. The fix is purely additive — it only updates
  `filtered_right_panel_files` in code paths where it was previously stale.
- **Performance:** Negligible. The re-filter is an O(n) iteration over
  `right_panel_files` with a HashSet membership check, reusing cached
  tantivy results. No I/O, no fresh tantivy search.
- **Backward compatibility:** No breaking changes. Existing behavior when
  no search is active is unaffected.
- **Edge cases:**
  - If `last_search_matches` is `None` (no prior search executed),
    `refilter_right_panel_files` is a no-op. The next
    `SearchQueryChanged` will repopulate it.
  - Filtered right-panel files are returned unsorted by the view (unlike
    `sorted_right_panel_files()` which applies the current sort order). Files
    added during an active search appear at the end of the filtered list.
    Applying sort/shuffle to the filtered list is a known UX inconsistency
    that pre-dates this fix and is not addressed here.
  - Files added while a search is active are filtered against the cached
    tantivy match set. If a file's metadata matches the search query but
    the file's path was not in the original tantivy index results (e.g., it
    existed outside the scanned directories), it will not appear in the
    filtered list. This is consistent with `perform_search()` behavior.

## 5. Files to Modify

| File                    | Change                                                        |
|------------------------|---------------------------------------------------------------|
| `src/gui/state.rs`     | Add `refilter_right_panel_files()` helper method              |
| `src/gui/update.rs`    | Add calls to `refilter_right_panel_files()` in 6 handlers; remove dead `recompute_filtered_right_panel_files()` function; add tests for all add/remove/clear paths |
