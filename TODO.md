# Implementation Plan: Fix Right Panel Search Filtering — Remove Search from Right Panel

Source: `docs/research/2025-07-fix-right-panel-search-filtering.md`

## Summary

The right panel is the **cumulative playlist** — it should always show every
file the user has added, regardless of the search query. Instead, when search
is active, the right panel only shows files whose paths happen to match.
This plan removes search filtering from the right panel entirely, undoing the
now-superseded `refilter_right_panel_files` approach and making the right
panel unconditionally display the full playlist.

| # | Commit message | Logical unit | Key deliverables | Tests |
|---|---|---|---|---|
| 1 | `fix: remove search branching from create_right_panel view` | View fix | `src/gui/right_panel.rs` | — |
| 2 | `fix: remove search branching from displayed_right_panel_files` | Export/play path fix | `src/gui/update.rs` | — |
| 3 | `fix: stop filtering right_panel_files in perform_search` | Search cleanup | `src/gui/state.rs` | — |
| 4 | `refactor: remove all filtered_right_panel_files writes from handlers` | Handler cleanup | `src/gui/update.rs` | — |
| 5 | `refactor: remove filtered_right_panel_files field and refilter method` | Dead code removal | `src/gui/state.rs` | — |
| 6 | `test: update tests for right panel ignoring search state` | Test update | `src/gui/update.rs` | Unit |

## Step Details

### Step 1 — Fix `create_right_panel` in `right_panel.rs`

Replace the search-branching logic that chooses between
`sorted_right_panel_files()` and `filtered_right_panel_files`. After this
change, the right panel view always displays `sorted_right_panel_files()`.

**Files modified:** `src/gui/right_panel.rs`

**Tests:** None needed — the test for this behavior is the integration-level
test in Step 6 that verifies the right panel ignores search state.

### Step 2 — Fix `displayed_right_panel_files` in `update.rs`

Replace the search-branching logic in this helper function used by Export/Play.
After this change, it always clones `right_panel_files` without filtering.

**Files modified:** `src/gui/update.rs`

**Tests:** None needed — covered by Step 6's `test_displayed_right_panel_files_ignores_search`.

### Step 3 — Remove `filtered_right_panel_files` population from `perform_search()`

Remove the 4-line block in `perform_search()` that filters
`right_panel_files` against `last_search_matches` and stores the result in
`filtered_right_panel_files`. The rest of `perform_search()` — which
populates `last_search_matches`, `filtered_root_nodes`, and
`filtered_tag_tree_roots` — remains unchanged; it is still needed for
left-panel filtering.

**Files modified:** `src/gui/state.rs`

**Tests:** None needed — existing tests still pass because the field is
still present and Step 1-2 already stopped reading from it. Step 6 handles
the behavioral assertion.

### Step 4 — Remove all `filtered_right_panel_files` writes from `update.rs`

Remove two categories of writes from message handlers:

1. **`app.refilter_right_panel_files()` calls** — from 6 handlers:
   `AddToRightPanel`, `AddDirectoryToRightPanel`, `AddTagNodeToRightPanel`,
   `RemoveFromRightPanel`, `RemoveDirectoryFromRightPanel`, `ClearRightPanel`

2. **`app.filtered_right_panel_files = Vec::new()` resets** — from 4 handlers:
   `SearchCleared`, `SearchQueryChanged` (empty-query branch),
   `RemoveTopDir` (no-search branch), `DirectoryAdded` (both blocks)

After this step, `update.rs` no longer writes to `filtered_right_panel_files`,
making the field dead storage.

**Files modified:** `src/gui/update.rs`

**Tests:** None needed — the field still exists so the code compiles. Step 6
adds behavioral tests.

### Step 5 — Remove dead code from `state.rs`

Remove the following from `FileTreeApp` and its `impl` block:

- **Field**: `pub filtered_right_panel_files: Vec<RightPanelFile>`
- **Method**: `pub(crate) fn refilter_right_panel_files(&mut self) { ... }`
- From `new()`: `filtered_right_panel_files: Vec::new(),`

All references were cleaned up in Step 4, so this should compile cleanly.

**Files modified:** `src/gui/state.rs`

**Tests:** Existing tests in `state.rs` (`test_new_app_search_defaults`,
`test_new_app_has_index`) reference `filtered_right_panel_files` in
assertions — these must be removed or updated. The serde-roundtrip test
(`test_tantivy_index_serde_skip`) likely doesn't reference it (field is
`#[serde(skip)]`).

### Step 6 — Update tests

Remove all tests that exercise `filtered_right_panel_files`:

- `test_toggle_search_mode_changes_filtered_right_panel` (~line 1130)
- `test_refilter_right_panel_noop_when_no_search` (~line 1210)
- `test_refilter_right_panel_noop_when_no_matches` (~line 1235)
- `test_add_to_right_panel_updates_filtered_during_search` (~line 1255)
- `test_add_file_not_in_matches_excluded_from_filtered` (~line 1270)
- `test_remove_from_right_panel_updates_filtered_during_search` (~line 1285)
- `test_clear_right_panel_updates_filtered_during_search` (~line 1320)
- Helper `app_with_search_and_matches` (~line 1200)

Remove `filtered_right_panel_files` from any state assertion tests that
reference it (e.g. in `test_new_app_search_defaults` in `state.rs`).

Add new tests:

1. `test_right_panel_shows_all_files_regardless_of_search` — file added
   before search remains visible after search is activated
2. `test_displayed_right_panel_files_ignores_search` — the export/play
   helper returns all files in both pre-search and active-search states

**Files modified:** `src/gui/update.rs`, `src/gui/state.rs`

**Tests:** Unit tests only (no property-based/integration/smoke needed).

## Build and Verify

After every step, run:

```sh
cargo test && cargo clippy && cargo fmt
```
