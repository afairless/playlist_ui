# Implementation Plan: Search Mode Filtering Bugfix

Source: `docs/research/search-mode-filtering-bugfix.md`

## Summary

Two independent bugs make search-mode cycling appear to have no effect:

1. The right panel (playlist) is **never filtered** by search at all.
2. The tag tree (`filter_tag_node`) filters by **label only**, ignoring both the search mode and file-level metadata.

The fix adds a `filtered_right_panel_files` model field (filtered in the update layer, not the view), passes the search mode into `filter_tag_node`, and extracts a shared `file_field_matches` utility for case-insensitive metadata field comparison.

| # | Commit message | Logical unit | Key deliverables | Tests |
|---|---|---|---|---|
| 1 | `chore: extract file_field_matches to utils` | Shared utility | `src/utils.rs` (+`file_field_matches`), `src/gui/left_panel.rs` (replace private `field_matches`) | unit |
| 2 | `feat: add right-panel search filtering` | Right-panel filtering | `src/gui/state.rs` (+field, init), `src/gui/update.rs` (+recompute, wire handlers), `src/gui/right_panel.rs` (use filtered + sorted) | unit, property |
| 3 | `feat: add mode awareness to filter_tag_node` | Tag tree mode filtering | `src/gui/left_panel.rs` (+mode param, label/file-path dispatch), `src/gui/update.rs` (pass mode) | unit, property |

### Sorting note

`filtered_right_panel_files` needs to be sorted before the view reads it. The recompute helper will apply the same sort logic as `sorted_right_panel_files()` using the app's current `right_panel_sort_column`, `right_panel_sort_order`, and `right_panel_shuffled` state. This keeps sorting in the update layer (where it belongs) rather than the view.

### Export scope note

`ExportRightPanelAsXspfTo` and `ExportAndPlayRightPanelAsXspf` should also use filtered files — only export what the user sees after search filtering.

---

## Step 1 — Extract `file_field_matches` to `src/utils.rs`

**What:** Add a `pub(crate) fn file_field_matches(value: &Option<String>, query: &str) -> bool` function to `src/utils.rs`. It performs a case-insensitive substring check, returning `false` when the value is `None`.

Then in `src/gui/left_panel.rs`:

- Replace the private `fn field_matches(...)` with `use crate::utils::file_field_matches;`
- Update `file_matches_mode` call sites: `field_matches(&meta.creator, query)` → `file_field_matches(&meta.creator, query)` etc.

**Tests (D.5 in the plan):**

- `test_file_field_matches_empty_query` — Empty query returns `true` for `Some` value
- `test_file_field_matches_none_value` — `None` value returns `false` for any non-empty query
- `test_file_field_matches_case_insensitive` — Case-insensitive matching
- `test_file_field_matches_no_match` — Non-matching returns `false`
- `test_file_field_matches_partial_match` — Substring match returns `true`

---

## Step 2 — Add right-panel search filtering

### 2a — Model field (`src/gui/state.rs`)

Add to `FileTreeApp`:

```rust
#[serde(skip)]
pub filtered_right_panel_files: Vec<RightPanelFile>,
```

Initialize in `FileTreeApp::new()`:

```rust
filtered_right_panel_files: Vec::new(),
```

### 2b — Update logic (`src/gui/update.rs`)

Add `recompute_filtered_right_panel_files(app: &FileTreeApp) -> Vec<RightPanelFile>`:

- Empty query → return all `app.right_panel_files` (sorted by current sort settings)
- Non-empty query → filter using `file_field_matches` for metadata fields (`Creator`, `Album`, `Title`, `Genre`), path/filename checks for `DirectoryPath`/`TrackFilename`, all-fields OR for `All` mode
- Apply sorting (same logic as `sorted_right_panel_files()`) to the filtered result

Wire into both `SearchQueryChanged` and `ToggleSearchMode` handlers:

```rust
app.filtered_right_panel_files = recompute_filtered_right_panel_files(app);
```

### 2c — View (`src/gui/right_panel.rs`)

Replace:

```rust
let displayed_files = app.sorted_right_panel_files();
```

With:

```rust
let displayed_files = &app.filtered_right_panel_files;
```

### Tests

**D.1 — Right-panel filtering unit tests:**

- `test_right_panel_filter_empty_query` — Empty query returns all files
- `test_right_panel_filter_genre_mode` — Genre mode matches genre field
- `test_right_panel_filter_title_mode` — Title mode, excludes other fields
- `test_right_panel_filter_album_mode` — Album mode
- `test_right_panel_filter_creator_mode` — Creator mode
- `test_right_panel_filter_path_mode` — DirectoryPath checks path string
- `test_right_panel_filter_filename_mode` — TrackFilename checks filename
- `test_right_panel_filter_all_mode` — All mode matches any metadata field
- `test_right_panel_filter_no_match` — Non-matching query returns empty
- `test_right_panel_filter_case_insensitive` — Case-insensitive
- `test_right_panel_filter_metadata_none_fields` — `None` fields excluded

**D.4 — Integration tests (right-panel specific):**

- `test_toggle_search_mode_changes_filtered_right_panel` — Verify filtering changes when mode toggles

**D.6 — Property-based tests (right-panel):**

- `prop_filter_right_panel_empty_query` — For any set of `RightPanelFile`s, filtering with empty query returns all files

---

## Step 3 — Add mode awareness to `filter_tag_node`

### 3a — Update `filter_tag_node` signature (`src/gui/left_panel.rs`)

Change from:

```rust
pub(crate) fn filter_tag_node(
    node: &TagTreeNode,
    query: &str,
) -> Option<TagTreeNode>
```

To:

```rust
pub(crate) fn filter_tag_node(
    node: &TagTreeNode,
    query: &str,
    mode: TextSearchMode,
) -> Option<TagTreeNode>
```

### 3b — Matching logic

- **Metadata modes** (`Creator`, `Album`, `Title`, `Genre`): match by label only (the tag tree's labels *are* the metadata values — no disk I/O needed)
- **`All` mode**: match by label OR file path / filename
- **`DirectoryPath` mode**: match by label OR file path substring
- **`TrackFilename` mode**: match by label OR filename substring

For leaf nodes: return `Some` if matches, `None` otherwise.
For non-leaf nodes: if label matches, keep all children; otherwise prune children recursively.

### 3c — Update call site (`src/gui/update.rs`)

In `recompute_filtered_tag_nodes`, pass `app.search_mode`:

```rust
.filter_map(|node| filter_tag_node(node, &app.search_query, app.search_mode))
```

### Tests

**D.3 — `filter_tag_node` mode-awareness tests:**

- `test_filter_tag_node_all_mode_matches_label` — All mode keeps node matching label
- `test_filter_tag_node_all_mode_matches_file_path` — All mode keeps node matching by path
- `test_filter_tag_node_label_matches_in_any_mode` — Label match sufficient in every mode
- `test_filter_tag_node_file_path_matches_in_genre_mode` — Genre mode via file paths
- `test_filter_tag_node_file_path_matches_in_album_mode` — Album mode
- `test_filter_tag_node_file_path_matches_in_creator_mode` — Creator mode
- `test_filter_tag_node_file_path_matches_in_title_mode` — Title mode
- `test_filter_tag_node_file_path_matches_in_path_mode` — DirectoryPath via file paths
- `test_filter_tag_node_file_path_matches_in_filename_mode` — TrackFilename via filenames
- `test_filter_tag_node_all_mode_checks_all_fields` — All mode checks label AND all fields

**D.4 — Integration tests (tag tree):**

- `test_toggle_search_mode_changes_filtered_root_nodes` — Verify `filtered_root_nodes` changes when mode toggles
- `test_toggle_search_mode_changes_filtered_tag_roots` — Verify `filtered_tag_tree_roots` changes when mode toggles

**D.6 — Property-based tests (tag tree):**

- `prop_filter_tag_node_empty_query` — For any `TagTreeNode`, `filter_tag_node(node, "", _) == Some(node)`
- `prop_filter_idempotent` — Running filter twice produces same result
