# Implementation Plan: Fix Filtered Node `file_count` Staleness During Search

Source: `docs/research/fix-filtered-node-file-count-staleness.md`

## Summary

When a text search is active, expanding a category node in the left panel
causes its `file_count` and highlight intensity to revert to unfiltered values.
The root cause is that `filter_tag_node` and `filter_file_node` in
`left_panel.rs` clone nodes without recalculating `file_count` after pruning
children. The fix adds a one-line recalculation in both functions, matching the
correct pattern already used by `prune_tag_node` and `prune_file_node` in
`tantivy_search.rs`.

## Plan

| # | Commit message | Logical unit | Key deliverables | Tests |
|---|---|---|---|---|
| 1 | `fix: recalculate file_count in filter_tag_node after child prune` | `filter_tag_node` fix + test | `src/gui/left_panel.rs` | Unit |
| 2 | `fix: recalculate file_count in filter_file_node after child prune` | `filter_file_node` fix + tests | `src/gui/left_panel.rs` | Unit |

### Step 1 — Fix `filter_tag_node` `file_count` recalculation

**Code change**: In `filter_tag_node`, after pruning children on a non-leaf
node whose label does not match, add `file_count` recalculation:

```rust
let mut cloned = node.clone();
cloned.children = filtered_children;
cloned.file_count =
    cloned.children.iter().map(|c| c.file_count).sum();
Some(cloned)
```

**Tests to add** (in `#[cfg(test)] mod tests` at the bottom of
`left_panel.rs`):

| Test | What it verifies |
|------|-----------------|
| `test_filter_tag_node_recalculates_file_count_on_child_prune` | Non-leaf with 3 children, search matches only 1 child → `file_count == 1` |
| `test_filter_tag_node_maintains_file_count_on_label_match` | Non-leaf whose label matches → `file_count` preserved (all children kept) |
| `test_filter_tag_node_nested_file_count_recalculation` | Two-level tree (genre→artists→tracks), only 1 track matches → intermediate node `file_count` correct |
| `test_filter_tag_node_path_mode_recalculates_file_count` | Parent non-matching, child matches via `DirectoryPath` mode → parent `file_count` correct after recalculation |

**Verify**: `cargo test`, `cargo clippy -- -D warnings`

### Step 2 — Fix `filter_file_node` `file_count` recalculation

**Code change**: In `filter_file_node`, after pruning children on a directory
node (both `node_matches` and `!node_matches` paths converge here), add
`file_count` recalculation:

```rust
let mut cloned = node.clone();
cloned.children = filtered_children;
cloned.file_count =
    cloned.children.iter().map(|c| c.file_count).sum();
Some(cloned)
```

**Tests to add** (in `#[cfg(test)] mod tests` at the bottom of
`left_panel.rs`):

| Test | What it verifies |
|------|-----------------|
| `test_filter_file_node_recalculates_file_count_on_child_prune` | Directory with 3 files, search matches only 1 (parent name does NOT match) → `file_count == 1` |
| `test_filter_file_node_recalculates_file_count_when_parent_matches` | Directory named `"jazz"` with 3 files, only 1 matches in metadata → `file_count == 1` (covers the `node_matches` code path) |
| `test_filter_file_node_maintains_file_count_when_empty_query` | Directory filtered with empty query → `file_count` equals total child count |

**Verify**: `cargo test`, `cargo clippy -- -D warnings`
