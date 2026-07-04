# Search Expansion Bugfix — Stale Expansion State in Filtered Trees

## Overview

When search is active on the left panel, clicking a tree node to expand it has
no visual effect. The underlying node *is* expanded in the original data
structure, but the view renders from a stale filtered copy whose `is_expanded`
field was never updated.

This bug affects both the **tag tree** (genre/creator/album hierarchy) and the
**file tree** (directory hierarchy). It was introduced when the text-search
feature added `filtered_root_nodes` and `filtered_tag_tree_roots` as separate
derived fields that are only recomputed on search-query or search-mode changes,
not on expansion toggles.

---

## Root-Cause Analysis

### Data Flow

```
User types query
    → SearchQueryChanged(query)
    → recompute_filtered_tag_nodes(app)     [creates clones with current is_expanded]
    → app.filtered_tag_tree_roots = result
    → view renders from filtered_tag_tree_roots   ✓

User clicks expand on a tag node
    → ToggleTagExpansion(path)
    → find_tag_node_mut(&mut app.tag_tree_roots, &path)   [mutates ORIGINAL tree]
    → node.is_expanded = !node.is_expanded                 ✓
    → … nothing recomputes filtered_tag_tree_roots         ✗
    → view still renders from stale filtered_tag_tree_roots with is_expanded: false
```

### Why the Filtered Clones Are Stale

Both `filter_file_node` and `filter_tag_node` (in `left_panel.rs`) clone nodes
via `node.clone()`, which preserves whatever `is_expanded` value the node had
*at the time of cloning*. These clones are only created when:

1. `SearchQueryChanged` fires — user types in the search box
2. `ToggleSearchMode` fires — user cycles through search modes

Expansion toggles (`ToggleExpansion`, `ToggleTagExpansion`) mutate the
original trees (`app.root_nodes` / `app.tag_tree_roots`) exclusively. Neither
handler triggers a recompute of the filtered derivatives.

### Concrete Example

1. User selects genre tag mode, searches for `"jazz"`
2. `filter_tag_node` finds the "Jazz" root (label matches) → `Some(node.clone())`
   → `is_expanded` is copied from original (likely `false` if not previously
   clicked, or `true` if it was)
3. User sees "Jazz (42)" with ▶ arrow, clicks it
4. `ToggleTagExpansion(["Jazz"])` fires → searches `app.tag_tree_roots`
   → finds "Jazz" → toggles `is_expanded` to `true` on the original
5. View rerenders, reads from `app.filtered_tag_tree_roots` → "Jazz" clone
   still has `is_expanded: false` → children not rendered
6. User clicks again → original toggles back to `false` → still no visible
   change (it was already collapsed in the view)

### Same Bug in File Tree

An identical issue exists for directory expansion during search:

```
User clicks expand on a filtered directory
    → ToggleExpansion(path)
    → expands_dirs.insert(path) or expands_dirs.remove(path)
    → restore_expansion_state(app.root_nodes, &app.expanded_dirs)  ✓
    → … nothing recomputes filtered_root_nodes                      ✗
    → view still renders from stale filtered_root_nodes
```

`ToggleExtension` (which rescans directory contents) also does not recompute
filtered trees, though the impact is less noticeable because the user likely
clears/retriggers the search after changing extensions.

---

## Proposed Fix

The fix has three parts, one per affected message handler. Each is a
one-line addition that recomputes the filtered tree from the (now up-to-date)
source tree.

### Part A — `ToggleTagExpansion` (tag tree)

In `update.rs`, after the expansion toggle on `app.tag_tree_roots`, add:

```rust
app.filtered_tag_tree_roots = recompute_filtered_tag_nodes(app);
```

This re-runs `filter_tag_node` on each root, which clones the newly toggled
node and captures the correct `is_expanded`.

### Part B — `ToggleExpansion` (file tree)

In `update.rs`, after `restore_expansion_state` on `app.root_nodes`, add:

```rust
app.filtered_root_nodes = recompute_filtered_nodes(app);
```

### Part C — `ToggleExtension` (file extension change)

In `update.rs`, after the rescan and `restore_expansion_state`, add both:

```rust
app.filtered_root_nodes = recompute_filtered_nodes(app);
app.filtered_tag_tree_roots = recompute_filtered_tag_nodes(app);
```

---

## Files to Modify

| File | Changes |
|------|---------|
| `src/gui/update.rs` | Three edits: add recompute calls to `ToggleTagExpansion`, `ToggleExpansion`, and `ToggleExtension` handlers |

No other files need changes. The filtering functions (`filter_file_node`,
`filter_tag_node`), the view layer (`left_panel.rs`, `render_node.rs`),
and the model (`state.rs`) are all correct — they just need the filtered
derivatives to be kept in sync.

---

## Implementation Order (Commit Plan)

### Commit 1 — Fix tag tree expansion during search

- In `ToggleTagExpansion` handler: add `app.filtered_tag_tree_roots =
  recompute_filtered_tag_nodes(app);` after the `is_expanded` toggle
- **Test:** Create a `FileTreeApp` with a non-empty `tag_tree_roots` and a
  non-empty `search_query`. Send `ToggleTagExpansion` for a node. Assert that
  `app.tag_tree_roots` has `is_expanded: true` (original) **and**
  `app.filtered_tag_tree_roots` has the corresponding cloned node with
  `is_expanded: true`.

### Commit 2 — Fix file tree expansion during search

- In `ToggleExpansion` handler: add `app.filtered_root_nodes =
  recompute_filtered_nodes(app);` after the `restore_expansion_state` loop
- **Test:** Create a `FileTreeApp` with a non-empty `root_nodes` and a
  non-empty `search_query`. Send `ToggleExpansion` for a directory. Assert
  that `app.filtered_root_nodes` reflects the expansion.

### Commit 3 — Fix extension change during search

- In `ToggleExtension` handler: add both recompute calls after the rescan
- **Test:** Verify that after `ToggleExtension`, `filtered_root_nodes` and
  `filtered_tag_tree_roots` are recomputed from the updated `root_nodes` /
  `tag_tree_roots`.

### Commit 4 — Clippy and final polish

- Run `cargo clippy --all-targets --all-features -- -D warnings`
- Run `cargo fmt`
- Run `cargo test`
- Update TODO.md

---

## Testing Plan

### Unit Tests (in `src/gui/update.rs` test module)

| Test | Description |
|------|-------------|
| `test_toggle_tag_expansion_during_search_updates_filtered` | Search active, toggle a tag node, verify both trees have `is_expanded: true` |
| `test_toggle_tag_expansion_no_search_preserves_filtered` | No search, toggle a tag node, verify `filtered_tag_tree_roots` stays in sync (cloned from identity) |
| `test_toggle_expansion_during_search_updates_filtered` | Search active, toggle a directory, verify `filtered_root_nodes` reflects expansion |
| `test_toggle_expansion_no_search_preserves_filtered` | No search, toggle a directory, verify `filtered_root_nodes` stays in sync (cloned from identity) — symmetric counterpart to the tag-tree no-search test |
| `test_toggle_extension_recomputes_filtered_trees` | Search active, toggle an extension, verify both filtered trees are refreshed |
| `test_toggle_extension_recomputes_filtered_trees_no_search` | No search, toggle an extension, verify both filtered trees are refreshed (cloned from identity) |
| `test_tag_expansion_nonmatching_parent_matching_child` | Search active, toggle expansion on a non-matching parent node whose children match, verify the filtered parent clone has `is_expanded: true` and matching children are rendered |

**Test fixtures**: Each test constructs a `FileTreeApp` with populated `tag_tree_roots` (e.g., `TagTreeNode { label, children, file_paths, is_expanded: false, file_count }`) or `root_nodes` (e.g., `FileNode::new_directory` / `FileNode::new_file`). Set `app.search_query` to a non-empty string, then dispatch the relevant `Message`. Assert on `app.filtered_tag_tree_roots` / `app.filtered_root_nodes` directly.

---

## Edge Cases and Considerations

### 1. Non-matching parent with matching children

When a parent node does not match the search query but some children do,
`filter_tag_node` returns a clone with `is_expanded` from the original.
If the user toggles expansion on the original tree (changing `is_expanded`
to `true`), the recompute correctly captures this. The filtered clone will
have `is_expanded: true` and its matching children will be rendered.

### 2. Rapid toggling (no mutex concern)

The recompute functions are synchronous (no I/O other than the already-present
metadata extraction in `filter_file_node`). Adding them to the toggle handlers
adds minimal latency — the same recompute already runs on every keystroke.

### 3. `filtered_root_nodes` already cloned on construction

In `state.rs`, the constructor initialises `filtered_root_nodes =
root_nodes.clone()`. After the fix, `ToggleExpansion` will replace this
with a freshly recomputed version, which is correct.

### 4. `filtered_tag_tree_roots` initialised as empty

In `state.rs`, `filtered_tag_tree_roots` starts as `Vec::new()` and is
populated on the first search. The `ToggleTagExpansion` recompute is a no-op
when `search_query` is empty (the filter functions return `Some(node.clone())`
for empty queries). This is safe and correct.

### 5. Performance

The recompute for tag nodes is cheap (label-only string comparison, no I/O).
The file-node recompute calls `extract_media_metadata` for each file, but
this already happens on every keystroke, so adding it to toggle handlers
does not change the worst-case cost — it merely shifts when the cost is
paid.
