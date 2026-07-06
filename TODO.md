# Implementation Plan: Right Panel Search Display Bug

Source: `docs/research/2025-07-right-panel-search-display-bug.md`

## Summary

When a text search query is active, files added to the right-panel playlist
do not appear in the UI because `filtered_right_panel_files` is stale. The
fix adds a `refilter_right_panel_files()` helper that reuses the cached
tantivy match set, wires it into all 6 handlers that mutate
`right_panel_files`, removes the dead `recompute_filtered_right_panel_files`
function, and adds unit tests.

| # | Commit message | Logical unit | Key deliverables | Tests |
|---|---|---|---|---|
| 1 | `feat: add refilter_right_panel_files helper to FileTreeApp` | Helper method | `src/gui/state.rs` | - |
| 2 | `fix: sync filtered_right_panel_files when right-panel modified during search` | Update handlers | `src/gui/update.rs` | - |
| 3 | `refactor: remove dead recompute_filtered_right_panel_files and its tests` | Dead code removal | `src/gui/update.rs` | - |
| 4 | `test: add tests for right-panel refilter during search` | Unit tests | `src/gui/update.rs` | Unit |

## Step Details

### Step 1 — Add `refilter_right_panel_files()` helper

Add a method on `FileTreeApp` in `state.rs` that re-filters
`filtered_right_panel_files` against `last_search_matches`, without
re-executing the tantivy search. No-op when no search query is active
or no cached matches exist.

**Files modified:** `src/gui/state.rs`

### Step 2 — Wire helper into 6 handlers in `update.rs`

Insert `app.refilter_right_panel_files();` immediately after mutations to
`right_panel_files` in these message handlers:

1. `AddToRightPanel` — after `app.right_panel_files.push(...)`
2. `AddDirectoryToRightPanel` — after the file-adding loop
3. `AddTagNodeToRightPanel` — after the file-adding loop
4. `RemoveFromRightPanel` — after `app.right_panel_files.retain(...)`
5. `RemoveDirectoryFromRightPanel` — after `app.right_panel_files.retain(...)`
6. `ClearRightPanel` — after `app.right_panel_files.clear()`

**Files modified:** `src/gui/update.rs`

### Step 3 — Remove dead code

Remove the `#[allow(dead_code)]` function
`recompute_filtered_right_panel_files()` (lines ~234-269 in `update.rs`)
and its `use crate::utils::file_field_matches;` import. Also remove the
existing tests that call `recompute_filtered_right_panel_files`:

- `test_right_panel_filter_empty_query`
- `test_right_panel_filter_genre_mode`
- `test_right_panel_filter_title_mode`
- `test_right_panel_filter_album_mode`
- `test_right_panel_filter_creator_mode`
- `test_right_panel_filter_path_mode`
- `test_right_panel_filter_filename_mode`
- `test_right_panel_filter_all_mode`
- `test_right_panel_filter_no_match`
- `test_right_panel_filter_case_insensitive`
- `test_right_panel_filter_metadata_none_fields`
- `test_toggle_search_mode_changes_filtered_right_panel`
- `test_toggle_search_mode_changes_filtered_tag_roots`

**Files modified:** `src/gui/update.rs`

### Step 4 — Add unit tests

Add tests in `update.rs` covering the refilter behavior using
**None-One-Many** principle:

- **None:** No-op when no search query / no cached matches
- **One:** Single add, single remove while search active; file in match
  set vs file outside match set
- **Many:** Bulk operations (`ClearRightPanel`)

**Files modified:** `src/gui/update.rs`
