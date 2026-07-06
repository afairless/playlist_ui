# Fix Filtered Node `file_count` Staleness During Search

## Overview

When a text search is active in the left panel, the category nodes (Directory,
Genre, Creator) initially show correct filtered counts and blue-highlight
intensities corresponding to the number of matching tracks. However, as soon as
a category is clicked on (expanded), the counts and highlights revert to the
**full, unfiltered** values, even though the search query is still active and
only a subset of tracks should be shown.

This is a staleness bug: after expansion, the filtered trees are recomputed
using `filter_file_node` and `filter_tag_node` (in `left_panel.rs`), which
**do not recalculate `file_count`** on cloned nodes whose children have been
pruned. The initial search uses `prune_file_node` and `prune_tag_node` (in
`tantivy_search.rs`), which **do** correctly recalculate `file_count`.

---

## Root-Cause Analysis

### Two Code Paths for Filtered Trees

The application has two separate code paths that produce filtered trees:

| Path | Entry Point | Functions Used | `file_count` Recalculated? |
|------|-------------|---------------|---------------------------|
| **Initial search** | `perform_search()` in `state.rs` | `prune_file_tree`, `prune_tag_node` (in `tantivy_search.rs`) | **Yes** |
| **Recompute after state change** | `recompute_filtered_nodes()`, `recompute_filtered_tag_nodes()` in `update.rs` | `filter_file_node`, `filter_tag_node` (in `left_panel.rs`) | **No** |

The initial search path works correctly. The recompute path has the bug.

### Data Flow (Bug Scenario)

```
1. User types "miles"
   → Message::SearchQueryChanged("miles")
   → perform_search()
   → tantivy finds matching paths
   → prune_tag_node() builds filtered_tag_tree_roots with CORRECT file_count
   → View renders "Jazz (3)" with correct blue shade    ✓

2. User clicks "Jazz" to expand it
   → Message::ToggleTagExpansion(["Jazz"])
   → toggles is_expanded on app.tag_tree_roots["Jazz"]
   → recompute_filtered_tag_nodes(app)
   → filter_tag_node("Jazz", "miles", ...)
   → "Jazz" label does NOT match "miles", so children are pruned
   → creates cloned node with filtered children
   → BUT file_count is copied from original (full) node: file_count = 50
   → View renders "Jazz (50)" with full blue shade        ✗ WRONG
```

### Detailed Code Trace in `filter_tag_node` (left_panel.rs)

```rust
// Non-leaf node whose label does NOT match the search query
let filtered_children: Vec<TagTreeNode> = node
    .children
    .iter()
    .filter_map(|child| filter_tag_node(child, query, mode))
    .collect();
if filtered_children.is_empty() {
    None
} else {
    let mut cloned = node.clone();           // clone — keeps original file_count
    cloned.children = filtered_children;     // children replaced, BUT...
    // file_count is STALE — still holds the original unfiltered value
    Some(cloned)
}
```

Compare with `prune_tag_node` (tantivy_search.rs) — correct version:

```rust
let pruned: Vec<TagTreeNode> = node
    .children
    .iter()
    .filter_map(|c| prune_tag_node(c, matches))
    .collect();
if pruned.is_empty() {
    None
} else {
    let mut cloned = node.clone();
    cloned.children = pruned;
    cloned.file_count = cloned.children.iter().map(|c| c.file_count).sum();
    //                     ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
    //                     CORRECTLY RECALCULATED
    Some(cloned)
}
```

### Affected Functions

Both filtering functions in `left_panel.rs` have this bug:

| Function | Affected Code Path | Fix |
|----------|-------------------|-----|
| `filter_tag_node` | Non-leaf, label doesn't match → pruned children | Add `cloned.file_count = cloned.children.iter().map(\|c\| c.file_count).sum();` |
| `filter_tag_node` | Non-leaf, label **does** match → `node.clone()` | No change needed (all children kept, original count is correct) |
| `filter_file_node` | Directory (all paths where children are filtered) | Add `cloned.file_count = cloned.children.iter().map(\|c\| c.file_count).sum();` |
| `filter_file_node` | File node | No change needed (file_count is always 1) |

> **Note:** In `filter_file_node`, children are *always* recursively filtered
> regardless of whether `node_matches` is true or false — both code paths
> converge on the same `Some(cloned)` return with `cloned.children =
> filtered_children`.  The `file_count` recalculation is therefore needed for
> **all** directory returns, not only the "children pruned but parent doesn't
> match" case.

### Callers Affected

The recompute functions are called in these message handlers in `update.rs`:

| Handler | Recompute Call | When Bug Manifests |
|---------|---------------|-------------------|
| `ToggleTagExpansion` | `recompute_filtered_tag_nodes(app)` | User expands a tag category during search |
| `ToggleExpansion` | `recompute_filtered_nodes(app)` | User expands a directory during search |

---

## Proposed Fix

Add `file_count` recalculation in `filter_tag_node` and `filter_file_node`
after pruning children, matching the pattern already used in `prune_tag_node`
and `prune_file_node`.

### Fix A — `filter_tag_node` (left_panel.rs, ~line 326)

Change the non-leaf pruned-children path from:

```rust
let mut cloned = node.clone();
cloned.children = filtered_children;
Some(cloned)
```

to:

```rust
let mut cloned = node.clone();
cloned.children = filtered_children;
cloned.file_count =
    cloned.children.iter().map(|c| c.file_count).sum();
Some(cloned)
```

### Fix B — `filter_file_node` (left_panel.rs, ~line 220)

Change the directory pruned-children path from:

```rust
let mut cloned = node.clone();
cloned.children = filtered_children;
Some(cloned)
```

to:

```rust
let mut cloned = node.clone();
cloned.children = filtered_children;
cloned.file_count =
    cloned.children.iter().map(|c| c.file_count).sum();
Some(cloned)
```

### Files to Modify

| File | Changes |
|------|---------|
| `src/gui/left_panel.rs` | Two edits: add `file_count` recalculation to `filter_tag_node` and `filter_file_node` |

No other files need changes. The view layer (`render_node.rs`, `left_panel.rs`
view functions) already reads `node.file_count` correctly — it just needs the
underlying data to be accurate.

---

## Testing Plan

### Existing Tests That Verify Correct Behavior

These existing tests in `tantivy_search.rs` already prove that `prune_tag_node`
and `prune_file_node` recalculate `file_count` correctly:

- `test_prune_tag_node_file_count_updated` — verifies `prune_tag_node` produces
  correct `file_count` after partial pruning
- `test_prune_file_tree_file_count_updated` — verifies `prune_file_tree`
  produces correct `file_count` after partial pruning
- `test_prune_idempotency` — verifies idempotency of `file_count` after prune

### New Unit Tests to Add (in `left_panel.rs` test module)

| Test | Description |
|------|-------------|
| `test_filter_tag_node_recalculates_file_count_on_child_prune` | Non-leaf node with 3 children, search for label matching only 1 child. Verify the returned node has `file_count == 1`, not 3. |
| `test_filter_tag_node_maintains_file_count_on_label_match` | Non-leaf node whose label matches the search. Verify the returned node preserves full `file_count` since all children are kept. |
| `test_filter_file_node_recalculates_file_count_on_child_prune` | Directory with 3 files, search matching only 1 (**parent name does NOT match**). Verify the returned directory node has `file_count == 1`. |
| `test_filter_file_node_recalculates_file_count_when_parent_matches` | Directory **named** `"jazz"` with 3 files, only 1 of which matches `"jazz"` in metadata (`TextSearchMode::All`). Verify `file_count == 1` (not 3, the original). Exercises the `node_matches` code path. |
| `test_filter_file_node_maintains_file_count_when_empty_query` | Directory filtered with empty query. Verify `file_count` equals total child count. |
| `test_filter_tag_node_nested_file_count_recalculation` | Two-level tag tree (genre→artists→tracks). Search matches only 1 track. Verify intermediate nodes (artist, genre) have correct `file_count`. |
| `test_filter_tag_node_path_mode_recalculates_file_count` | Parent node with non-matching label, child matching via file path in `TextSearchMode::DirectoryPath`. Verify parent `file_count` is correct after recalculation (mode-agnostic regression guard). |

---

## Implementation Order

### Commit 1 — Fix `file_count` recalculation in `filter_tag_node` and `filter_file_node`

- Add `cloned.file_count = cloned.children.iter().map(|c| c.file_count).sum();`
  in both functions where children are pruned.
- Add unit tests for both functions (see testing plan above).
- Run `cargo test` and `cargo clippy`.

### Commit 2 — (Optional) Coverage: `filter_file_node` directory-name-match path

- In `filter_file_node`, children are *always* recursively filtered regardless
  of whether `node_matches` is true or false.  The same `file_count`
  recalculation applied in Commit 1 covers both paths; no additional code
  change is needed.  Add `test_filter_file_node_recalculates_file_count_when_parent_matches`
  to explicitly cover the directory-name-match path and prevent regressions.

---

## Edge Cases

### 1. Deeply nested tag trees

`filter_tag_node` is recursive. When a non-leaf node is pruned, its
`file_count` should be recalculated as the sum of its (already correctly
filtered) children's `file_count`s. Since the children are recursively filtered
first, their `file_count`s are already correct (after the fix), so the
parent-level sum is also correct.

### 2. File nodes

File nodes always have `file_count == 1`. `filter_file_node` returns
`Some(node.clone())` or `None` for file nodes — no recalculation needed.

### 3. Non-leaf label matching in tag tree

When a non-leaf node's label matches the search, all children are kept
(`Some(node.clone())`). The original `file_count` is correct since all
children are present. When a non-leaf node does NOT match but its children do,
only matching children survive, and `file_count` must be recalculated (this is
the bug fix).

### 4. Empty query

When `search_query` is empty, both filter functions return `Some(node.clone())`
immediately — all children and counts are preserved. No recalculation needed.

---

## Verification Checklist

- [ ] `cargo test` passes all tests
- [ ] `cargo clippy -- -D warnings` passes
- [ ] Manual test: search for a term, verify counts are reduced, expand a
  category, verify counts STAY reduced (don't revert to full counts)
- [ ] Manual test: clear search, verify counts revert to full counts
- [ ] Manual test: search in Genre mode, expand a genre, verify child counts
  are correct
- [ ] Manual test: search in Creator mode, expand a creator, verify child
  counts are correct
- [ ] Manual test: search in Directory mode, expand a directory, verify child
  counts are correct
