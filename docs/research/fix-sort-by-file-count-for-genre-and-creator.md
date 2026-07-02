# Fix: Sort by File Count for Genre and Creator Tag Trees

## Motivation

The left panel's **Sort by File Count** mode correctly orders directory nodes
(children of a root directory) by their `file_count` in descending order, but
fails to reorder the **root-level nodes** of the genre and creator tag trees.
When the user toggles to Genre or Creator selection mode, the top-level nodes
(genre names or creator names) always appear in alphabetical order regardless
of the active sort mode.

This makes Sort by File Count functionally useless for tag-based browsing —
the user sees genres/creators in alphabetical order rather than by size,
defeating the purpose of the feature.

## Root Cause Analysis

### 1. Root-level nodes are never sorted

The rendering entry point for the tag tree in
`src/gui/left_panel.rs` — `create_left_panel_tag_tree_browser` —
iterates `app.tag_tree_roots` in its natural (insertion) order:

```rust
for node in &app.tag_tree_roots {
    trees = trees.push(render_tag_node(
        node,
        0,
        vec![],
        tree_browser_style.directory_row_size,
        app.left_panel_sort_mode,   // passed to children only
        flat_button_style,
        max_count,
    ));
```

There is no sorting of `tag_tree_roots` before this loop, regardless of
`app.left_panel_sort_mode`.

### 2. Render function only sorts children

`render_tag_node` in `src/gui/render_node.rs` correctly sorts `node.children`
for all three sort modes:

- `Alphanumeric` → sorts children by label
- `ModifiedDate` → sorts children by modification time
- `FileCount` → sorts children by `file_count` descending

But the function receives an individual node and operates on **its
children**. The node itself is never sorted relative to its siblings.

### 3. Tag tree roots are built from BTreeMap

Both `build_genre_tag_tree` and `build_creator_tag_tree` in
`src/gui/media_metadata.rs` store intermediate data in `BTreeMap`, which
iterates in **alphabetical key order**. This means `tag_tree_roots` is
always ordered alphabetically by genre/creator name, irrespective of
`file_count`.

### 4. File tree works by different anatomy

In the directory tree, `create_left_panel_file_tree_browser` iterates
`app.root_nodes`, which correspond to top-level added directories (typically
1-3 roots). The visible sortable items are the **children** of these roots
(subdirectories and files), and those children **are** sorted by
`render_file_node`. The user sees the effect immediately.

In the tag tree, the roots (genres/creators) **are** the visible top-level
items. Since roots are never sorted, changing the sort mode has no visible
effect on the top level.

## Scope

| Affected code | File | Role |
|---|---|---|
| `create_left_panel_tag_tree_browser` | `src/gui/left_panel.rs` | Entry point — renders tag_tree_roots unsorted |
| `render_tag_node` | `src/gui/render_node.rs` | Sorts children correctly — no change needed |
| `build_genre_tag_tree` / `build_creator_tag_tree` | `src/fs/media_metadata.rs` | Builds roots from BTreeMap — already sets file_count correctly |

## Plan

### Step 1: Sort `tag_tree_roots` before rendering

**File**: `src/gui/left_panel.rs`
**Function**: `create_left_panel_tag_tree_browser`

Before the root-iteration loop, sort a local copy of `tag_tree_roots`
according to `app.left_panel_sort_mode`.

Use the same sorting logic already present in `render_tag_node`:

```rust
// Alphanumeric: sort by label ascending
tag_tree_roots.sort_by(|a, b| {
    a.label.to_lowercase().cmp(&b.label.to_lowercase())
});

// FileCount: sort by file_count descending, then label ascending
tag_tree_roots.sort_by(|a, b| {
    b.file_count.cmp(&a.file_count)
        .then_with(|| a.label.to_lowercase().cmp(&b.label.to_lowercase()))
});

// ModifiedDate: sort by modification time of first file path, newest first
tag_tree_roots.sort_by(|a, b| {
    let a_time = a.file_paths
        .first()
        .and_then(|p| std::fs::metadata(p).ok())
        .and_then(|m| m.modified().ok());
    let b_time = b.file_paths
        .first()
        .and_then(|p| std::fs::metadata(p).ok())
        .and_then(|m| m.modified().ok());
    b_time.cmp(&a_time) // newest first
});
```

**Rationale for sorting at render time instead of caching**:

- iced re-renders on every state change anyway, so the overhead is a single
  sort per render cycle
- The tag tree is small (<1000 nodes), making this negligible
- The sort mode can change without rebuilding the tree, so cached order would
  need invalidation logic

**Key design decision**: sort a **clone** of `tag_tree_roots` to avoid
mutating the app state during the view function (pure-Elm principle). Iced
re-renders use immutable references to the model (`&FileTreeApp`), so we
can only sort a copy. The iced widget tree is rebuilt each frame, so
sorting a local copy is the correct approach.

```rust
fn create_left_panel_tag_tree_browser(
    app: &FileTreeApp,
    tree_browser_style: TreeBrowserStyle,
    flat_button_style: impl Fn(
        &iced::Theme,
        iced::widget::button::Status,
    ) -> iced::widget::button::Style
    + Copy
    + 'static,
) -> iced::widget::Column<'_, Message> {
    let max_count =
        app.tag_tree_roots.iter().map(|n| n.file_count).max().unwrap_or(0);

    // Sort a local copy of roots according to the current sort mode
    let mut sorted_roots = app.tag_tree_roots.clone();
    sort_tag_tree_roots(&mut sorted_roots, app.left_panel_sort_mode);

    let mut trees = column![];
    for node in &sorted_roots {
        trees = trees.push(render_tag_node(
            node,
            0,
            vec![],
            tree_browser_style.directory_row_size,
            app.left_panel_sort_mode,
            flat_button_style,
            max_count,
        ));
        trees = trees.push(Space::with_height(
            tree_browser_style.tree_row_height,
        ));
    }
    trees
}

/// Sorts a mutable slice of TagTreeNodes according to the given sort mode.
fn sort_tag_tree_roots(
    roots: &mut [TagTreeNode],
    sort_mode: LeftPanelSortMode,
) {
    match sort_mode {
        LeftPanelSortMode::Alphanumeric => {
            roots.sort_by(|a, b| {
                a.label.to_lowercase().cmp(&b.label.to_lowercase())
            });
        },
        // NOTE: Non-leaf tag tree nodes have empty file_paths, so for
        // root-level nodes this comparator always sees None == None and
        // produces no effective sort. This is a pre-existing limitation
        // shared with the child-level sort in render_tag_node (see
        // Risks and Trade-offs). The `.then_with` fallback ensures roots
        // are at least sorted alphabetically when timestamps are missing.
        LeftPanelSortMode::ModifiedDate => {
            roots.sort_by(|a, b| {
                let a_time = a
                    .file_paths
                    .first()
                    .and_then(|p| std::fs::metadata(p).ok())
                    .and_then(|m| m.modified().ok());
                let b_time = b
                    .file_paths
                    .first()
                    .and_then(|p| std::fs::metadata(p).ok())
                    .and_then(|m| m.modified().ok());
                b_time
                    .cmp(&a_time)
                    .then_with(|| {
                        a.label
                            .to_lowercase()
                            .cmp(&b.label.to_lowercase())
                    })
            });
        },
        LeftPanelSortMode::FileCount => {
            roots.sort_by(|a, b| {
                b.file_count
                    .cmp(&a.file_count)
                    .then_with(|| {
                        a.label.to_lowercase().cmp(&b.label.to_lowercase())
                    })
            });
        },
    }
}
```

**Placement**: Define `sort_tag_tree_roots` as a private helper function in
`left_panel.rs` (alongside `create_left_panel_tag_tree_browser`). This keeps
it co-located with its only caller.

### Step 2: Add comprehensive tests

**File**: `src/gui/left_panel.rs` (new test module)

Add tests for the new `sort_tag_tree_roots` helper:

| Test | Scenario | Assertion |
|---|---|---|
| `test_sort_roots_alphanumeric` | Three nodes with labels "z_genre", "a_genre", "m_genre" | Sorted: "a_genre", "m_genre", "z_genre" |
| `test_sort_roots_file_count_descending` | Three nodes with file_counts 50, 100, 30 | Sorted by count descending: 100-count, 50-count, 30-count |
| `test_sort_roots_file_count_tiebreaker` | Two nodes with same file_count (50 each), labels "b" and "a" | Sorted by label: "a", "b" |
| `test_sort_roots_file_count_same_as_directory` | Nodes with file_counts 100, 50, 10, and a file with count 1, compared against expected child-level sort order | Confirms root-level FileCount ordering matches child-level FileCount ordering |
| `test_sort_roots_file_count_no_panic_on_single` | Single root node | Does not panic, returns single element |
| `test_sort_roots_file_count_no_panic_on_empty` | Empty roots slice | Does not panic, returns empty |
| `test_sort_roots_alphanumeric_mixed_case` | Nodes "Z_genre", "a_genre", "M_genre" (mixed case) | Sorted case-insensitively: "a_genre", "M_genre", "Z_genre" |
| `test_sort_roots_modified_date_empty_paths` | Nodes with empty file_paths, ModifiedDate mode | Does not panic; no crash on None timestamps |
| `test_sort_roots_modified_date_fallback_alpha` | Nodes with empty file_paths, ModifiedDate mode | Fallback to alphabetical order by label when file_paths are empty |

Also add a regression test to verify that root-level sorting is wired into
`create_left_panel_tag_tree_browser` (not just isolated in the helper). Since
`iced::Column` children are type-erased and not publicly introspectable,
the simplest approach is to enhance `test_render_tag_node_sorted_by_file_count`
by constructing expanded tag tree roots with known file counts and passing
`FileCount` mode — this complements the existing child-level sort test and
exercises the full `create_left_panel_tag_tree_browser` path without
panicking. The core sort correctness is already covered by the direct unit
tests on `sort_tag_tree_roots` above.

### Step 3: Update module docstrings

- `src/gui/left_panel.rs`: Update the `//!` docstring to mention
  `sort_tag_tree_roots` in the module description (it is a private helper,
  not part of the public API) and clarify that roots are sorted before
  rendering.
- `src/gui/render_node.rs`: If no interface change, no doc update needed.

### Step 4: Run full test suite and verify

```sh
cargo test
cargo clippy
cargo fmt --check
```

## Risks and Trade-offs

### Cloning tag_tree_roots every frame

Sorting requires mutation, but the view function receives an immutable
`&FileTreeApp`. The current code already creates a `Vec<TagTreeNode>` copy
at various points (the `sorted_right_panel_files` method clones the entire
`right_panel_files` vec). The tag tree is typically small (tens to low
hundreds of nodes), so cloning `tag_tree_roots` each frame is acceptable.

If performance becomes a concern, the alternative is to sort in the
**update handler** when the sort mode or selection mode changes, storing
the sorted roots in a cached field on `FileTreeApp`. This is a more complex
change that requires cache invalidation on every tree rebuild.

### ModifiedDate heuristic for root nodes

The root-level ModifiedDate sort uses the same logic as the child-level sort
in `render_tag_node`: it picks the modification time of the **first file
path** in the node's `file_paths`. For roots (genres/creators), `file_paths`
is empty because the root node's files are in its descendant leaf nodes.

**This is a bug**: the root nodes of the tag tree have `file_paths: vec![]`
because `build_genre_tag_tree` and `build_creator_tag_tree` only set
`file_paths` on leaf (track) nodes:

```rust
// build_genre_tag_tree: roots are built with empty file_paths
roots.push(TagTreeNode {
    label: genre,
    children: artist_nodes,
    file_paths: vec![],          // ← empty!
    is_expanded: false,
    file_count: genre_file_count,
});
```

This means **ModifiedDate sorting won't work for root-level nodes** because
they have no file paths. The sort comparator will compare `None` with `None`,
which gives `Equal` for every pair, resulting in no effective sort.

To mitigate this, `sort_tag_tree_roots` includes a fallback in the
`ModifiedDate` arm: when both nodes have unavailable modification times (as
is always the case for root-level tag tree nodes), it falls through to
alphabetical order by label rather than leaving roots unsorted. This is
documented with an inline comment in the code.

The same issue exists for the child-level `render_tag_node` `ModifiedDate`
sort for genre/creator/album nodes — but those nodes ALSO have empty
`file_paths`. The only nodes with populated `file_paths` are the leaf
(track) nodes.

**This is a pre-existing issue** with the ModifiedDate sort in the tag tree
and is out of scope for the current plan. The plan adds root-level sorting
using the same heuristic as the child-level sort; both are equally broken
for ModifiedDate mode. Fixing ModifiedDate for tag trees (both root and
child level) is a separate task that would require either:

1. Populating `file_paths` on all nodes (not just leaves) during tree
   construction
2. Walking the subtree to find the latest modification time among leaf
   descendants during sorting

### FileCount at root level works correctly

The root nodes DO have `file_count` set correctly (genres sum artists,
artists sum albums, albums sum tracks). The `sort_tag_tree_roots` helper
uses `file_count` directly, so FileCount sorting at the root level will
work correctly.

## Verification Steps

1. Build and run the application
2. Add a directory with audio files having varied genre and creator tags
3. Switch to Genre selection mode
4. Toggle sort mode to "Sort: File Count"
5. Verify genre root nodes are ordered by file count descending
6. Expand some genres — verify child (artist/album) nodes are also ordered
   by file count descending (this already works)
7. Switch to Creator selection mode
8. Verify creator root nodes are ordered by file count descending
9. Toggle back to "Sort: Name" — verify alphabetical order is restored for
   both Genre and Creator modes
10. Run `cargo test` to confirm no regressions

## Implementation Order

| Step | Description | Files touched | Commit message |
|---|---|---|---|
| 1 | Add `sort_tag_tree_roots` helper and call it in `create_left_panel_tag_tree_browser` | `src/gui/left_panel.rs` | `feat: sort tag tree roots by current sort mode before rendering` |
| 2 | Add comprehensive unit tests for root-level sorting | `src/gui/left_panel.rs` | `test: add tests for tag tree root-level sorting` |
| 3 | Update module docstrings, run `cargo test` + `cargo clippy` | `src/gui/left_panel.rs` | `docs: update left_panel module docstring` |

## Appendix: Current Code Provenance

The tag tree's child-level FileCount sorting was already implemented in
`render_tag_node` (commit for "sort-left-panel-by-file-count" feature). The
`file_count` field was added to `TagTreeNode` in a prior commit (for
"file-count-indicator-in-left-panel"). The root-level sorting was omitted
from both these patches because the sort-at-render-time design only
targeted the recursive child sort inside `render_tag_node`, not the
top-level iteration in `create_left_panel_tag_tree_browser`.

This omission is not a code bug per se — all the sorting mechanisms work
correctly — but rather a **coverage gap** in the sorting pipeline. The
child-level sort handles expanded nodes correctly, and the root-level sort
was simply never wired up.
