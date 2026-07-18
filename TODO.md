# Implementation Plan: Random Tracks from Category

Source: `docs/research/random-tracks-from-category.md`

| # | Commit message | Logical unit | Key deliverables | Tests |
|---|---|---|---|---|
| 1 | `feat: add random_count fields and message variants to state` | State fields & Message variants | `src/gui/state.rs` | Unit |
| 2 | `feat: add N text input to left panel menu row` | N input widget | `src/gui/left_panel.rs` | — |
| 3 | `feat: handle RandomCountChanged input validation` | Input validation | `src/gui/update.rs` | Unit |
| 4 | `refactor: thread random_count into render_file_node and render_tag_node` | Parameter plumbing | `src/gui/render_node.rs`, `src/gui/left_panel.rs` | — |
| 5 | `feat: add random-N context menu entries for directory and tag nodes` | Context menu entries | `src/gui/render_node.rs` | — |
| 6 | `feat: implement AddRandomDirectoryToRightPanel` | Random directory addition | `src/gui/update.rs` | Unit |
| 7 | `feat: implement AddRandomTagNodeToRightPanel` | Random tag-node addition | `src/gui/update.rs` | Unit |
| 8 | `chore: run CI suite and manual verification` | Validation & CI | — | Integration, Smoke |

---

## Step Details

### Step 1 — State fields & Message variants

**File:** `src/gui/state.rs`

**Changes:**

- Add two `#[serde(skip)]` fields to `FileTreeApp`:
  - `pub random_count: usize`
  - `pub random_count_input: String`
- Initialize in `FileTreeApp::new()`:
  - `random_count: 6`
  - `random_count_input: "6".to_string()`
- Add three variants to `Message`:
  - `RandomCountChanged(String)`
  - `AddRandomTagNodeToRightPanel(Vec<String>)`
  - `AddRandomDirectoryToRightPanel(PathBuf)`

**Tests:**

- `test_new_app_random_count_default` — verify `random_count == 6` and `random_count_input == "6"` on a fresh `FileTreeApp::new()`

**Verification:** `cargo check` passes.

---

### Step 2 — N text input widget

**File:** `src/gui/left_panel.rs`

**Changes:**

- In `create_left_panel_menu_row`, append to the existing `row!`:
  - A `text("N:").size(menu_style.text_size)` label, styled with `menu_style.text_color` (cyan)
  - A `text_input` widget bound to `app.random_count_input`, producing `Message::RandomCountChanged` on input, with `.width(50)`

**Resulting row layout:**

```
[←] [Add Directory] [Sort: Name]  N: [__]
```

**Tests:** None — compilation check only (visual verification in Step 8).

**Verification:** `cargo check` passes.

---

### Step 3 — RandomCountChanged validation

**File:** `src/gui/update.rs`

**Changes:**

- Add match arm for `Message::RandomCountChanged(new_text)`:
  - Parse `new_text` as `usize`
  - If `Ok(n)` and `n > 0` → accept: `random_count = n`, `random_count_input = new_text`
  - If `Ok(0)` → revert: `random_count_input = random_count.to_string()`
  - If `Err` (invalid or overflow) → revert: `random_count_input = random_count.to_string()`
  - Return `Task::none()`

**Edge cases handled:**

- Empty string → parse fails → reverts
- `"0"` → parse succeeds but `n == 0` → reverts
- `"abc"` → parse fails → reverts
- `"12"` → parse succeeds, `n > 0` → accepted
- Leading zeros `"06"` → `parse::<usize>()` yields `Ok(6)` → accepted, raw input preserved
- `"99999999999999999999"` (> `usize::MAX`) → parse fails → reverts

**Tests (in `src/gui/update.rs`):**

- `test_random_count_valid_input` — `"12"` updates both fields
- `test_random_count_invalid_text_reverts` — `"abc"` reverts
- `test_random_count_zero_reverts` — `"0"` reverts
- `test_random_count_empty_reverts` — `""` reverts
- `test_random_count_leading_zeros_accepted` — `"06"` sets count to 6, input stays `"06"`
- `test_random_count_overflow_reverts` — huge number > `usize::MAX` reverts

**Verification:** `cargo test` passes.

---

### Step 4 — Thread `random_count` into render functions

**Files:** `src/gui/render_node.rs`, `src/gui/left_panel.rs`

**Changes to `render_node.rs`:**

- Add `random_count: usize` parameter to `render_file_node` (after `sort_mode`, before `flat_button_style`)
- Add `random_count: usize` parameter to `render_tag_node` (after `sort_mode`, before `flat_button_style`)
- Pass `random_count` through recursive calls unchanged

**Changes to `left_panel.rs`:**

- In `create_left_panel_file_tree_browser`, pass `app.random_count` to `render_file_node`
- In `create_left_panel_tag_tree_browser`, pass `app.random_count` to `render_tag_node`

**Tests:** None — compilation check only. All existing tests must still compile.

**Verification:** `cargo check` passes.

---

### Step 5 — Context menu entries for random-N addition

**File:** `src/gui/render_node.rs`

**Changes to `render_file_node` (Directory branch):**

- In the `ContextMenu` closure, add a second button below the existing "Add all files…":

  ```
  "Add {random_count} random files to right panel" → AddRandomDirectoryToRightPanel(dir_path)
  ```

- Capture `random_count` by value (usize is Copy) inside the `move ||` closure
- Clone `dir_path` inside the closure following the existing pattern

**Changes to `render_tag_node` (non-leaf branch):**

- In the `ContextMenu` closure, add a second button below the existing "Add all files…":

  ```
  "Add {random_count} random files to right panel" → AddRandomTagNodeToRightPanel(path)
  ```

- Capture `random_count` by value inside the `move ||` closure
- Clone `path` inside the closure following the existing pattern

**Tests:** None — compilation check only (visual verification in Step 8).

**Verification:** `cargo check` passes.

---

### Step 6 — Implement `AddRandomDirectoryToRightPanel`

**File:** `src/gui/update.rs`

**Changes:**

- Add match arm for `Message::AddRandomDirectoryToRightPanel(dir_path)`:
  1. Set `right_panel_shuffled = false`
  2. Find the directory node in `app.root_nodes` (same as existing `AddDirectoryToRightPanel`)
  3. Collect all files recursively via `collect_files_recursively`
  4. Filter by active search matches (`app.last_search_matches`) if present
  5. Compute `n = min(random_count, files.len())`
  6. If `n < files.len()`, use `rand::seq::SliceRandom::partial_shuffle` to randomly select `n` files, then truncate
  7. Deduplicate against `right_panel_files`, extract metadata via `extract_media_metadata`, and push

**Tests (in `src/gui/update.rs`):**

- `test_add_random_directory_selects_subset` — N ≤ file count selects exactly N
- `test_add_random_directory_all_when_n_exceeds` — N > file count adds all
- `test_add_random_directory_respects_search_filter` — search filter limits results
- `test_add_random_directory_no_filter_when_search_inactive` — all files pass through when `last_search_matches` is `None`
- `test_add_random_directory_no_duplicates` — already-present files not added again
- `test_add_random_directory_resets_shuffle` — `right_panel_shuffled` set to false
- `test_add_random_directory_n_zero_adds_none` — when `random_count` is 0, no files added (defensive)

**Verification:** `cargo test` passes.

---

### Step 7 — Implement `AddRandomTagNodeToRightPanel`

**File:** `src/gui/update.rs`

**Changes:**

- Add match arm for `Message::AddRandomTagNodeToRightPanel(path)`:
  1. Set `right_panel_shuffled = false`
  2. Find the tag node in `app.tag_tree_roots` via `find_tag_node_mut`
  3. Collect all files via `collect_tag_node_files`
  4. Filter by active search matches (`app.last_search_matches`) if present
  5. Compute `n = min(random_count, files.len())`
  6. If `n < files.len()`, use `partial_shuffle` + `truncate` for random selection
  7. Deduplicate against `right_panel_files`, extract metadata, and push

**Tests (in `src/gui/update.rs`):**

- `test_add_random_tag_node_selects_subset` — N ≤ total selects exactly N
- `test_add_random_tag_node_all_when_n_exceeds` — N > file count adds all
- `test_add_random_tag_node_respects_search_filter` — search filter limits results
- `test_add_random_tag_node_no_filter_when_search_inactive` — all files pass through when `last_search_matches` is `None`
- `test_add_random_tag_node_no_duplicates` — already-present files not added again
- `test_add_random_tag_node_n_zero_adds_none` — when `random_count` is 0, no files added (defensive)

**Verification:** `cargo test` passes.

---

### Step 8 — CI suite and manual verification

**Commands:**

```
cargo fmt --check
cargo clippy
cargo test
cargo build --release
```

**Manual test checklist:**

1. Launch app, verify "N: 6" appears on top row
2. Change N to "3", verify field updates
3. Type "abc", verify it reverts to "3"
4. Type "0", verify it reverts to "3"
5. Right-click a directory node, verify both "Add all files…" and "Add 3 random files…" appear
6. Choose "Add 3 random files…", verify exactly 3 tracks (or all if <3) appear in right panel
7. Switch to Genre mode, repeat steps 5–6
8. Switch to Creator mode, repeat steps 5–6
9. Activate a search filter, verify random addition respects the filter

---

## Design Notes

- `rand` is already a dependency (v0.9.2). No Cargo.toml changes needed.
- `random_count` and `random_count_input` are `#[serde(skip)]` — not persisted, resets to 6 on every launch.
- Using `partial_shuffle` + `truncate` for O(N) random selection — matches the established `rand::seq::SliceRandom` pattern used by `ShuffleRightPanel`.
- Separate `random_count_input` string allows iced's controlled `text_input` to show intermediate keystrokes before validation.
