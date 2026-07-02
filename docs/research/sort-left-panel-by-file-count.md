# Sort Left Panel by File Count

## Motivation

The left panel currently supports two sort modes: **Alphanumeric** (Name) and
**Date Modified**. Both are structural (how items are ordered) but give no
insight into the *size* of each category. Adding a **sort by file count**
mode lets the user see the largest genres, artists, albums, or directories
at a glance — the most information-dense categories bubble to the top.

This is especially useful in the tag tree (genre/creator) where nodes have no
inherent filesystem ordering, and users typically want to find the biggest
genres or artists first.

## Current State

### Sort mode enum (`state.rs`)

```rust
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
pub enum LeftPanelSortMode {
    #[default]
    Alphanumeric,
    ModifiedDate,
}
```

### Update handler (`update.rs`)

Cycles between the two variants:

```rust
Message::ToggleLeftPanelSortMode => {
    app.left_panel_sort_mode = match app.left_panel_sort_mode {
        LeftPanelSortMode::Alphanumeric => LeftPanelSortMode::ModifiedDate,
        LeftPanelSortMode::ModifiedDate => LeftPanelSortMode::Alphanumeric,
    };
    Task::none()
},
```

### Button label (`left_panel.rs`)

```rust
let sort_mode_label = match app.left_panel_sort_mode {
    LeftPanelSortMode::Alphanumeric => "Sort: Name",
    LeftPanelSortMode::ModifiedDate => "Sort: Date Modified",
};
```

### Sorting logic (`render_node.rs`)

**`render_file_node`** already receives `sort_mode: LeftPanelSortMode` and
applies it when a directory node is expanded:

- `Alphanumeric`: directories first (by name), then files (by name)
- `ModifiedDate`: directories first (by modification time, newest first),
  then files (by modification time, newest first)

Both keep `NodeType::Directory` before `NodeType::File` as the primary sort key.

**`render_tag_node`** does **not** accept a `sort_mode` parameter and does
**not** sort children at all — they render in whatever order the tree was
built (which from `BTreeMap` happens to be alphabetical, but this is not
guaranteed by any explicit sort).

### Call chain: tag tree

```
create_left_panel_tag_tree_browser()
  → render_tag_node(node, depth, path, directory_row_size, flat_button_style, max_count)
    → for child in &node.children { render_tag_node(child, ...) }   // unsorted!
```

No sort mode is threaded through this path.

## Scope

| Tree type | Sorted currently? | Will sort? | Data structure |
|---|---|---|---|
| Directory tree (`render_file_node`) | Yes (Alphanumeric, ModifiedDate) | Yes + FileCount | `FileNode` |
| Tag tree (`render_tag_node`) | No | Yes (all 3 modes) | `TagTreeNode` |

Adding sort-by-file-count to the directory tree alone would be trivial but
incomplete. The plan adds the new mode to both trees, which also means
**threading sort mode into `render_tag_node` for the first time**.

## Sorting Rules for FileCount Mode

### Directory tree (`render_file_node`)

| Primary sort | Secondary sort |
|---|---|
| `Directory` before `File` | — |
| Directories: `file_count` descending | alphabetical name  |
| Files: all have `file_count == 1` | alphabetical name (no change) |

This preserves the invariant that directories always appear before files
within a parent directory.

### Tag tree (`render_tag_node`)

| Primary sort | Secondary sort |
|---|---|
| Non-leaf nodes: `file_count` descending | alphabetical label |
| Leaf nodes: all have `file_count == 1` | alphabetical label |

Since every leaf `TagTreeNode` has `file_count == 1` (each leaf holds exactly
one track), sorting by file count has no effect on leaf order — leaves will
always fall back to alphabetical ordering. Non-leaf nodes (genre, artist,
album) will be ordered by how many tracks they contain, descending.

For all other modes (Alphanumeric, ModifiedDate), tag tree children will
sort alphabetically by label (Alphanumeric) or by modification time of the
first file path (ModifiedDate) as a reasonable default.

## Implementation Steps

### Step 1: Add `FileCount` variant to `LeftPanelSortMode`

**File**: `src/gui/state.rs`

- Add `FileCount` variant to the `LeftPanelSortMode` enum
- Update the module docstring in `state.rs` (the `//!` header that lists
  `LeftPanelSortMode`)
- Update the module docstring in `mod.rs` (the public API list)

```rust
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
pub enum LeftPanelSortMode {
    #[default]
    Alphanumeric,
    ModifiedDate,
    FileCount,
}
```

The ordering of variants in the cycle will be changed in Step 2.

**Tests**:

- `test_left_panel_sort_mode_cycles_through_three_modes` — verify that
  toggling cycles Alphanumeric → ModifiedDate → FileCount → Alphanumeric
- `test_left_panel_sort_mode_default` — verify default is still Alphanumeric

### Step 2: Update the toggle cycle (and fix existing test)

**File**: `src/gui/update.rs`

Expand the match to include the new `FileCount` variant so the cycle becomes:

```
Alphanumeric → ModifiedDate → FileCount → Alphanumeric
```

**⚠️ The existing `test_toggle_left_panel_sort_mode` test in `view.rs`
asserts a 2-mode cycle and WILL FAIL after this change.** Update it in the
same commit: replace its assertions with the 3-mode cycle test defined in
Step 7.

```rust
Message::ToggleLeftPanelSortMode => {
    app.left_panel_sort_mode = match app.left_panel_sort_mode {
        LeftPanelSortMode::Alphanumeric => LeftPanelSortMode::ModifiedDate,
        LeftPanelSortMode::ModifiedDate => LeftPanelSortMode::FileCount,
        LeftPanelSortMode::FileCount => LeftPanelSortMode::Alphanumeric,
    };
    Task::none()
},
```

### Step 3: Update the button label

**File**: `src/gui/left_panel.rs`

Add the new label:

```rust
let sort_mode_label = match app.left_panel_sort_mode {
    LeftPanelSortMode::Alphanumeric => "Sort: Name",
    LeftPanelSortMode::ModifiedDate => "Sort: Date Modified",
    LeftPanelSortMode::FileCount => "Sort: File Count",
};
```

### Step 4: Add FileCount sorting to `render_file_node`

**File**: `src/gui/render_node.rs`

Add a new `LeftPanelSortMode::FileCount` arm in the `match sort_mode` block
inside the `NodeType::Directory` branch, after the `ModifiedDate` arm:

```rust
LeftPanelSortMode::FileCount => {
    indices.sort_by(|&i, &j| {
        let a = &node.children[i];
        let b = &node.children[j];
        match (a.node_type.clone(), b.node_type.clone()) {
            (NodeType::Directory, NodeType::File) => {
                std::cmp::Ordering::Less
            },
            (NodeType::File, NodeType::Directory) => {
                std::cmp::Ordering::Greater
            },
            _ => {
                // Directories: sort by file_count descending (largest first)
                // Files: both have count 1, so falls back to alphabetical
                let count_cmp = b.file_count.cmp(&a.file_count);
                count_cmp.then_with(|| {
                    a.name.to_lowercase().cmp(&b.name.to_lowercase())
                })
            },
        }
    });
},
```

**Why `b.file_count.cmp(&a.file_count)`?** — This gives descending order
(largest first), as specified by the user.

### Step 5: Thread sort mode through tag tree rendering and update callers

**Files**: `src/gui/render_node.rs`, `src/gui/left_panel.rs`

This step must be done as a single atomic change — adding the `sort_mode`
parameter to `render_tag_node` without immediately updating every caller
will break the build.

#### 5a: Add `sort_mode` parameter to `render_tag_node`

Change the signature from:

```rust
pub(crate) fn render_tag_node(
    node: &TagTreeNode,
    depth: usize,
    path: Vec<String>,
    directory_row_size: u16,
    flat_button_style: ...,
    max_count: usize,
) -> Element<'_, Message> {
```

To:

```rust
pub(crate) fn render_tag_node(
    node: &TagTreeNode,
    depth: usize,
    path: Vec<String>,
    directory_row_size: u16,
    sort_mode: LeftPanelSortMode,
    flat_button_style: ...,
    max_count: usize,
) -> Element<'_, Message> {
```

#### 5b: Add sorting of children before iterating

Replace the unsorted loop:

```rust
if node.is_expanded {
    for child in &node.children {
        content = content.push(render_tag_node(
            child,
            depth + 1,
            new_path.clone(),
            directory_row_size,
            flat_button_style,
            max_count,
        ));
    }
}
```

With a sorted one:

```rust
if node.is_expanded {
    let mut indices: Vec<usize> = (0..node.children.len()).collect();
    match sort_mode {
        LeftPanelSortMode::Alphanumeric => {
            indices.sort_by(|&i, &j| {
                node.children[i]
                    .label
                    .to_lowercase()
                    .cmp(&node.children[j].label.to_lowercase())
            });
        },
        LeftPanelSortMode::ModifiedDate => {
            indices.sort_by(|&i, &j| {
                let a_time = node.children[i]
                    .file_paths
                    .first()
                    .and_then(|p| std::fs::metadata(p).ok())
                    .and_then(|m| m.modified().ok());
                let b_time = node.children[j]
                    .file_paths
                    .first()
                    .and_then(|p| std::fs::metadata(p).ok())
                    .and_then(|m| m.modified().ok());
                b_time.cmp(&a_time) // newest first
            });
        },
        LeftPanelSortMode::FileCount => {
            indices.sort_by(|&i, &j| {
                let count_cmp = node.children[j]
                    .file_count
                    .cmp(&node.children[i].file_count);
                count_cmp.then_with(|| {
                    node.children[i]
                        .label
                        .to_lowercase()
                        .cmp(&node.children[j].label.to_lowercase())
                })
            });
        },
    }
    for &i in &indices {
        content = content.push(render_tag_node(
            &node.children[i],
            depth + 1,
            new_path.clone(),
            directory_row_size,
            sort_mode,
            flat_button_style,
            max_count,
        ));
    }
}
```

Note: For `ModifiedDate` in the tag tree, we use the modification time of
the first file path in each node as a proxy. This is a reasonable heuristic
since tag tree nodes don't have a direct filesystem path. If a node has no
file paths, it falls to the end.

#### 5c: Update the recursive call to pass `sort_mode`

Done in the code above — the recursive call to `render_tag_node` inside the
loop already passes the new parameter.

#### 5d: Update `create_left_panel_tag_tree_browser` to pass sort mode

**File**: `src/gui/left_panel.rs`

The call site currently passes no sort mode because `render_tag_node` didn't
accept one. Now it must pass `app.left_panel_sort_mode`:

```rust
fn create_left_panel_tag_tree_browser(
    app: &FileTreeApp,
    tree_browser_style: TreeBrowserStyle,
    flat_button_style: ...,
) -> iced::widget::Column<'_, Message> {
    let max_count = ...;
    let mut trees = column![];
    for node in &app.tag_tree_roots {
        trees = trees.push(render_tag_node(
            node,
            0,
            vec![],
            tree_browser_style.directory_row_size,
            app.left_panel_sort_mode,   // NEW
            flat_button_style,
            max_count,
        ));
        trees = trees.push(Space::with_height(tree_browser_style.tree_row_height));
    }
    trees
}
```

### Step 6: Update tests

#### 6a: New tests in `view.rs` / `update.rs`

| Test | What it checks |
|---|---|
| `test_left_panel_sort_mode_cycles_through_three_modes` | Toggle cycles Alphanumeric → ModifiedDate → FileCount → Alphanumeric |
| `test_left_panel_sort_mode_default` | Default is still `Alphanumeric` |
| `test_render_file_node_sorted_by_file_count` | Directory children sorted by count descending, directories before files |
| `test_render_tag_node_sorted_by_file_count` | Tag tree children sorted by count descending |
| `test_render_tag_node_unsorted_children_sorted_alphabetically_now` | Verifies that tag tree children now sort alphabetically in `Alphanumeric` mode (regression test — this is new behaviour) |

#### 6b: Existing tests to check

Run `cargo test`. Most existing tests should remain green, but note:

- **`test_toggle_left_panel_sort_mode`** — **will be removed** (replaced by
  `test_left_panel_sort_mode_cycles_through_three_modes` in Step 2's commit)
- All other existing tests should continue to pass unchanged

### Step 7: Update module docstrings

Update the `//!` module docstrings in:

- `src/gui/state.rs` — update the `LeftPanelSortMode` description line from
  `"alphanumeric or modified-date sort"` to
  `"alphanumeric, modified-date, or file-count sort"`
- `src/gui/mod.rs` — same update in the public API list
- `src/gui/render_node.rs` — update public API list if adding `sort_mode` to
  `render_tag_node` changes the documented interface

## Risks and Trade-offs

1. **Tag tree ModifiedDate sort is heuristic**: Tag tree nodes don't have a
   filesystem path; we use `first().file_paths[0]` modification time. A genre
   node's time is the modification time of the first track file (alphabetically).
   This is consistent but not perfectly accurate. An alternative would be
   using the max (newest) modification time of all child file paths, but that
   would require scanning all descendant paths.

2. **Filesystem I/O in tag tree ModifiedDate sort comparator**: The
   `ModifiedDate` arm calls `std::fs::metadata()` inside the sort comparator,
   resulting in up to `O(n log n)` filesystem calls for a tree of `n` nodes.
   For the typically small tag tree (<1000 nodes) this is negligible, but if
   performance becomes an issue, modification times should be pre-fetched
   before sorting. Errors from unreadable paths are silently swallowed via
   `.ok()`, causing those nodes to fall to the end of the sort order.

3. **Tag tree size**: The tag tree is typically small (<1000 nodes), so
   sorting at render time is negligible. If performance becomes an issue,
   sort keys could be pre-computed and stored on `TagTreeNode`.

4. **BTreeMap insertion order**: The current tag trees are built from
   `BTreeMap`, which yields keys in alphabetical order. Adding explicit
   sorting in `render_tag_node` means the tag tree now respects the sort
   mode, which is more predictable regardless of the underlying map
   implementation.

## Future Considerations

- **Sort indicator in the UI**: Show a small arrow or indicator next to
  "Sort: File Count" to indicate the current active mode at a glance
  (similar to right-panel column headers).
- **Ascending/descending toggle**: The file count sort is hard-coded to
  descending. A future iteration could add a secondary toggle (or extend
  `LeftPanelSortMode` with direction) to allow ascending order.
- **Unified sort state**: The sort mode is currently left-panel-only. If a
  future refactor unifies tree types, the sort mode could be moved to a more
  central location.
