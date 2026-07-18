# Random Tracks from Category — Implementation Plan

## Feature Summary

Add a new context-menu option on category nodes (Directory, Genre, Creator) to
add a random subset of N tracks to the right panel, instead of all tracks.  N
is controlled by a text-input field on the left panel's top row, defaulting to
6.

## User Story

1. User sees an `N:` text field on the left-panel top row, pre-filled with 6.
2. User can change N by typing a positive integer; invalid input reverts.
3. Right-clicking a category (directory, genre, or creator) shows two options:
   - "Add all files to right panel" (existing)
   - "Add **N** random files to right panel" (new, with current N substituted)
4. Choosing the random option selects N tracks uniformly at random from all
   tracks under that category, adds them to the right panel, and respects the
   active search filter (just like the existing "add all" path).
5. If N exceeds the number of available tracks, all tracks are added.

---

## Affected Files & Change Summary

| File | Change |
|------|--------|
| `src/gui/state.rs` | Add `random_count`, `random_count_input` fields; add `RandomCountChanged`, `AddRandomTagNodeToRightPanel`, `AddRandomDirectoryToRightPanel` message variants |
| `src/gui/left_panel.rs` | Add `N:` label + text input to `create_left_panel_menu_row` |
| `src/gui/render_node.rs` | Add new context-menu entry on directory + tag non-leaf nodes; accept `random_count` parameter |
| `src/gui/update.rs` | Handle `RandomCountChanged` validation; implement `AddRandomTagNodeToRightPanel` and `AddRandomDirectoryToRightPanel`; add corresponding unit tests |

---

## Detailed Steps

Each step is one small, compilable, testable increment.  Commit after each step
with a conventional-commit message.

### Step 1 — Add state fields and Message variants

**File:** `src/gui/state.rs`

Add to `FileTreeApp`:

```rust
#[serde(skip)]
pub random_count: usize,
#[serde(skip)]
pub random_count_input: String,
```

Initialize in `FileTreeApp::new()`:

```rust
random_count: 6,
random_count_input: "6".to_string(),
```

Add to the `Message` enum:

```rust
RandomCountChanged(String),
AddRandomTagNodeToRightPanel(Vec<String>),
AddRandomDirectoryToRightPanel(PathBuf),
```

**Verification:** `cargo check` passes.  Add a unit test in `src/gui/state.rs`
(`test_new_app_random_count_default`) that verifies `random_count` defaults
to 6 and `random_count_input` defaults to `"6"`.

---

### Step 2 — Add the N input field to the left panel menu row

**File:** `src/gui/left_panel.rs`

In `create_left_panel_menu_row`, after the sort-mode button, append to the
existing `iced::widget::row!`:

- A `text("N:").size(menu_style.text_size)` label, styled like the existing
  sort-mode button text (cyan colour via `menu_style.text_color`).
- A `text_input` widget whose value is `&app.random_count_input`, producing
  `Message::RandomCountChanged` on input, with `.width(50)` to keep it narrow.

The row should look like:

```
[←] [Add Directory] [Sort: Name]  N: [__]
```

No changes to the signature of `create_left_panel_menu_row` — it already takes
`&FileTreeApp`.

**Verification:** `cargo check` passes.  Visual test: the field appears, typing
produces `RandomCountChanged` messages (even though the handler does nothing
yet).

---

### Step 3 — Handle RandomCountChanged validation

**File:** `src/gui/update.rs`

Add an arm for `Message::RandomCountChanged(new_text)`:

```rust
Message::RandomCountChanged(new_text) => {
    if let Ok(n) = new_text.parse::<usize>() {
        if n > 0 {
            app.random_count = n;
            app.random_count_input = new_text;
        } else {
            // 0 is not a positive integer — revert
            app.random_count_input =
                app.random_count.to_string();
        }
    } else {
        // Not a valid integer — revert
        app.random_count_input =
            app.random_count.to_string();
    }
    Task::none()
},
```

**Edge cases handled:**

- Empty string → parse fails → reverts to last valid count
- "0" → parse succeeds but n == 0 → reverts
- "abc" → parse fails → reverts
- "12" → parse succeeds, n > 0 → accepted
- Leading zeros like "06" → `parse::<usize>()` yields `Ok(6)` → accepted

**Verification:** `cargo test` passes.  Add unit tests in `src/gui/update.rs`:

- `test_random_count_valid_input` — "12" updates both `random_count` and
  `random_count_input`
- `test_random_count_invalid_text_reverts` — "abc" reverts both fields
- `test_random_count_zero_reverts` — "0" reverts both fields
- `test_random_count_empty_reverts` — "" reverts both fields
- `test_random_count_leading_zeros_accepted` — "06" sets `random_count` to 6
  and `random_count_input` to "06" (raw input is preserved; normalization to
  "6" happens on the next revert)
- `test_random_count_overflow_reverts` — a number exceeding `usize::MAX`
  (e.g., "99999999999999999999") fails to parse and reverts both fields

---

### Step 4 — Thread random_count into render functions

**Files:** `src/gui/render_node.rs`, `src/gui/view.rs`, `src/gui/left_panel.rs`

**`render_node.rs`:**

- Add parameter `random_count: usize` to both `render_file_node` and
  `render_tag_node`. Place it after the `sort_mode` parameter and before
  `flat_button_style`.
- Recursive calls pass `random_count` through unchanged.
- The context-menu label for the new entry will use this value (added in the
  next step).

**`left_panel.rs`:**

- In `create_left_panel_file_tree_browser`, pass `app.random_count` to
  `render_file_node`.
- In `create_left_panel_tag_tree_browser`, pass `app.random_count` to
  `render_tag_node`.

**`view.rs`:**

- No signature change needed — `create_left_panel` is called from `view` and
  already receives `app`.  The plumbing is internal to `left_panel.rs`.

**Verification:** `cargo check` passes.  All existing tests compile.

---

### Step 5 — Add context menu entries for random-N addition

**Files:** `src/gui/render_node.rs`

#### Directory nodes (`render_file_node`, `NodeType::Directory` branch)

The current context menu has one entry:

```
"Add all files to right panel" → AddDirectoryToRightPanel(dir_path)
```

Add a second entry below it:

```
"Add {random_count} random files to right panel" → AddRandomDirectoryToRightPanel(dir_path)
```

The label is dynamic: `format!("Add {random_count} random files to right panel")`.

Capture `random_count` in both closures so the label reads correctly even if
the user changes N after the menu was created (the menu closure captures at
creation time, but menus are short-lived so this is acceptable).  Note:
`iced_aw::ContextMenu` closures run `move ||`, so `random_count` (a `usize`,
which is `Copy`) is captured directly.  The `dir_path` / `path` variables must
be `.clone()`-d inside the closure, matching the existing pattern for the
"Add all" entry.

#### Tag non-leaf nodes (`render_tag_node`, non-leaf branch)

The current context menu has one entry:

```
"Add all files to right panel" → AddTagNodeToRightPanel(path)
```

Add a second entry below it:

```
"Add {random_count} random files to right panel" → AddRandomTagNodeToRightPanel(path)
```

**Verification:** `cargo check` passes.  Visual test: right-click any category;
both options appear.

---

### Step 6 — Implement AddRandomDirectoryToRightPanel

**File:** `src/gui/update.rs`

Add an arm for `Message::AddRandomDirectoryToRightPanel(dir_path)`:

```rust
Message::AddRandomDirectoryToRightPanel(dir_path) => {
    app.right_panel_shuffled = false;
    for root in app.root_nodes.iter().flatten() {
        if let Some(node) = find_node_by_path(root, &dir_path) {
            let mut files = Vec::new();
            collect_files_recursively(node, &mut files);
            // Filter by active search, if any
            if let Some(ref matches) = app.last_search_matches {
                files.retain(|f| matches.contains(f));
            }
            // Random subset
            let n = app.random_count.min(files.len());
            if n < files.len() {
                use rand::seq::SliceRandom;
                let mut rng = rand::rng();
                files.partial_shuffle(&mut rng, n);
                files.truncate(n);
            }
            for file in files {
                if !app.right_panel_files.iter().any(|f| f.path == file) {
                    let meta = extract_media_metadata(&file);
                    app.right_panel_files.push(RightPanelFile {
                        path: file,
                        creator: meta.creator,
                        album: meta.album,
                        title: meta.title,
                        genre: meta.genre,
                        duration_ms: meta.duration_ms,
                    });
                }
            }
        }
    }
    Task::none()
},
```

Logic:

1. Collect all files under the directory node (using unfiltered
   `app.root_nodes`, same as `AddDirectoryToRightPanel`).
2. Filter by active search matches (same as `AddDirectoryToRightPanel`).
   When `last_search_matches` is `None` (no active search), all files
   pass through unfiltered.
3. Take `min(N, total)` random files using `rand::seq::SliceRandom::partial_shuffle`.
4. Add each (deduplicating) to the right panel.

**Verification:** `cargo test` passes.  Add tests in `src/gui/update.rs`:

- `test_add_random_directory_selects_subset` — N ≤ file count selects exactly
  N files (spot-check: `right_panel_files.len() == N`)
- `test_add_random_directory_all_when_n_exceeds` — N > file count adds all
- `test_add_random_directory_respects_search_filter` — search filter applied
  (when `last_search_matches` is `Some`)
- `test_add_random_directory_no_filter_when_search_inactive` — when
  `last_search_matches` is `None`, all files pass through unfiltered
- `test_add_random_directory_no_duplicates` — files already in the right
  panel are not added again
- `test_add_random_directory_resets_shuffle` — `right_panel_shuffled` set to
  false
- `test_add_random_directory_n_zero_adds_none` — when `random_count` is 0
  (shouldn't happen in normal use due to validation, but defensive check),
  no files are added

---

### Step 7 — Implement AddRandomTagNodeToRightPanel

**File:** `src/gui/update.rs`

Add an arm for `Message::AddRandomTagNodeToRightPanel(path)`:

```rust
Message::AddRandomTagNodeToRightPanel(path) => {
    app.right_panel_shuffled = false;
    if let Some(node) = find_tag_node_mut(&mut app.tag_tree_roots, &path) {
        let mut files = Vec::new();
        collect_tag_node_files(node, &mut files);
        // Filter by active search, if any
        if let Some(ref matches) = app.last_search_matches {
            files.retain(|f| matches.contains(f));
        }
        // Random subset
        let n = app.random_count.min(files.len());
        if n < files.len() {
            use rand::seq::SliceRandom;
            let mut rng = rand::rng();
            files.partial_shuffle(&mut rng, n);
            files.truncate(n);
        }
        for file in files {
            if !app.right_panel_files.iter().any(|f| f.path == file) {
                let meta = extract_media_metadata(&file);
                app.right_panel_files.push(RightPanelFile {
                    path: file,
                    creator: meta.creator,
                    album: meta.album,
                    title: meta.title,
                    genre: meta.genre,
                    duration_ms: meta.duration_ms,
                });
            }
        }
    }
    Task::none()
},
```

Logic mirrors `AddRandomDirectoryToRightPanel` but operates on tag tree nodes.

1. Collect all files under the tag node (using unfiltered
   `app.tag_tree_roots`).
2. Filter by active search matches. When `last_search_matches` is `None`
   (no active search), all files pass through unfiltered.
3. Take `min(N, total)` random files using `rand::seq::SliceRandom::partial_shuffle`.
4. Add each (deduplicating) to the right panel.

**Verification:** `cargo test` passes.  Add tests in `src/gui/update.rs`:

- `test_add_random_tag_node_selects_subset` — selects exactly N when N ≤ total
- `test_add_random_tag_node_all_when_n_exceeds` — N > file count adds all
- `test_add_random_tag_node_respects_search_filter` — search filter applied
  (when `last_search_matches` is `Some`)
- `test_add_random_tag_node_no_filter_when_search_inactive` — when
  `last_search_matches` is `None`, all files pass through unfiltered
- `test_add_random_tag_node_no_duplicates` — files already in the right
  panel are not added again
- `test_add_random_tag_node_n_zero_adds_none` — when `random_count` is 0,
  no files are added (defensive; validation prevents this in normal use)

---

### Step 8 — End-to-end validation

Run the full CI suite:

```sh
cargo fmt --check
cargo clippy
cargo test
cargo build --release
```

Manually test:

1. Launch the app, verify "N: 6" appears on the top row.
2. Change N to "3", verify the field updates.
3. Type "abc", verify it reverts to "3".
4. Type "0", verify it reverts to "3".
5. Right-click a directory, verify both "Add all files…" and "Add 3 random files…" appear.
6. Choose "Add 3 random files…", verify exactly 3 tracks (or all if <3) appear in the right panel.
7. Switch to Genre mode, repeat steps 5–6.
8. Switch to Creator mode, repeat steps 5–6.
9. Activate a search filter, verify random addition respects the filter.

---

## Design Decisions

### Why `partial_shuffle` instead of `choose_multiple`?

`rand::seq::SliceRandom::partial_shuffle` shuffles the first N elements in
place, then we truncate.  This is O(N) time vs O(total) for a full shuffle.
`choose_multiple` is also available but `partial_shuffle` + `truncate` is
idiomatic in the rand 0.9 API.

### Why a separate `random_count_input` String field?

`text_input` in iced is a *controlled* widget — its value is always equal to
the model's string.  If we only stored the parsed `usize`, we couldn't show
intermediate digits during typing (e.g., when typing "12", the "1" is a valid
intermediate).  Storing both the parsed value and the raw string lets us
validate on every keystroke while keeping the text input responsive.

### No persistence for `random_count`

The field is not persisted to disk — it resets to the default of 6 on every
launch.  This is the simplest approach and avoids schema migration concerns.
If persistence is desired later, `random_count` can be added to the serde
fields and the persistence logic in `FileTreeApp`.

### Raw input is preserved on valid entry

When the user types "06", `parse::<usize>()` yields `Ok(6)`, so
`random_count` becomes 6 and `random_count_input` becomes "06".  The raw
string is kept exactly as typed.  Normalization to "6" only occurs on the
next revert (e.g., if the user then types an invalid character).  This
preserves the editing experience without surprising the user with
auto-correction mid-typing.

### Context menu captures `random_count` by value

`iced_aw::ContextMenu` closures run `move ||`, so they capture `random_count`
at the moment the menu is created.  Context menus are short-lived (created on
right-click, destroyed after selection), so the captured value is always
current.  No stale-value concern in practice.
