# Implementation Plan: Option A — Skip Re-Filtering on Expand/Collapse

## Goal

Make expand/collapse operations in the left panel instant when a search
is active, by avoiding a full O(n) tree reclone on every toggle.
Expand/collapse should only touch `is_expanded` flags; filtering
should only run when the search query or mode changes.

## Background

- **Research doc:** `docs/research/expand-collapse-search-performance.md`
- **The bottleneck:** `ToggleExpansion` and `ToggleTagExpansion` both
  call `recompute_filtered_*` which deep-clones and re-filters ~32,000
  tree nodes. The filter query has not changed — only an `is_expanded`
  flag flipped — yet the entire tree is cloned.
- **Key insight:** `filter_file_node` and `filter_tag_node` do not
  inspect `is_expanded` anywhere. The filtered tree's structure is
  entirely independent of expansion state. There is no reason to
  re-filter when expansion toggles.

## Overview of Changes

Two handlers in `src/gui/update.rs` are modified:

1. **`ToggleExpansion`** (file-tree directories): Instead of
   `recompute_filtered_nodes`, apply `restore_expansion_state` to the
   existing `filtered_root_nodes` in-place.

2. **`ToggleTagExpansion`** (tag-tree Genre/Creator nodes): Instead of
   `recompute_filtered_tag_nodes`, find the corresponding node in
   `filtered_tag_tree_roots` by path and toggle its `is_expanded` flag
   in-place.

Estimated LOC change: ~20 lines modified, ~0 lines added net.

---

## Step 1: Modify `ToggleExpansion` handler

**File:** `src/gui/update.rs`, roughly line 303

### Current code

```rust
Message::ToggleExpansion(path) => {
    if app.expanded_dirs.contains(&path) {
        app.expanded_dirs.remove(&path);
    } else {
        app.expanded_dirs.insert(path);
    }
    for root in app.root_nodes.iter_mut().flatten() {
        restore_expansion_state(root, &app.expanded_dirs);
    }
    app.filtered_root_nodes = recompute_filtered_nodes(app);
    Task::none()
},
```

### New code

```rust
Message::ToggleExpansion(path) => {
    if app.expanded_dirs.contains(&path) {
        app.expanded_dirs.remove(&path);
    } else {
        app.expanded_dirs.insert(path);
    }
    // Sync expansion state into the unfiltered tree.
    for root in app.root_nodes.iter_mut().flatten() {
        restore_expansion_state(root, &app.expanded_dirs);
    }
    // Sync expansion state into the already-filtered tree in-place.
    // No re-filter is needed — the filter query hasn't changed and
    // filter_{file,tag}_node do not inspect is_expanded.
    for filtered in app.filtered_root_nodes.iter_mut().flatten() {
        restore_expansion_state(filtered, &app.expanded_dirs);
    }
    Task::none()
},
```

### Rationale

- `restore_expansion_state` is a simple bool-set walk — no allocation,
  no cloning. It already exists and handles the case where a path in
  `expanded_dirs` doesn't correspond to any node in the filtered tree
  (the path is silently skipped).
- The second walk over `filtered_root_nodes` is an O(n) bool-set
  operation, negligible compared to the O(n) deep-clone it replaces.
- `recompute_filtered_nodes` is no longer called from this handler.
  The function itself is retained with `#[allow(dead_code)]` — it is
  used by tests in the same module (lines 954, 968) as a setup helper.

---

## Step 2: Modify `ToggleTagExpansion` handler

**File:** `src/gui/update.rs`, roughly line 754

### Current code

```rust
Message::ToggleTagExpansion(path) => {
    if let Some(node) =
        find_tag_node_mut(&mut app.tag_tree_roots, &path)
    {
        node.is_expanded = !node.is_expanded;
    }
    app.filtered_tag_tree_roots = recompute_filtered_tag_nodes(app);
    Task::none()
},
```

### New code

```rust
Message::ToggleTagExpansion(path) => {
    // Toggle in the unfiltered tree and capture the new state.
    let new_state = if let Some(node) =
        find_tag_node_mut(&mut app.tag_tree_roots, &path)
    {
        node.is_expanded = !node.is_expanded;
        node.is_expanded
    } else {
        return Task::none();
    };
    // Apply the same toggle to the already-filtered tree in-place.
    // The filtered tree shares the same path structure as the
    // unfiltered tree; nodes pruned by the filter are simply absent.
    if let Some(node) = find_tag_node_mut(
        &mut app.filtered_tag_tree_roots,
        &path,
    ) {
        node.is_expanded = new_state;
    }
    Task::none()
},
```

### Rationale

- `find_tag_node_mut` already exists and traverses a `&mut [TagTreeNode]`
  by label path. It works for both `tag_tree_roots` and
  `filtered_tag_tree_roots`.
- The borrow checker requires us to drop the mutable borrow on
  `tag_tree_roots` before accessing `filtered_tag_tree_roots`. We
  extract `new_state` into a local variable to achieve this.
- If the node was pruned from the filtered tree (no children matched
  the search), `find_tag_node_mut` returns `None` — we simply skip the
  toggle. This is correct: the node isn't visible in the filtered view.
- `recompute_filtered_tag_nodes` is no longer called from this handler.
  The function is retained with `#[allow(dead_code)]` — it is used by
  tests in the same module (lines 861, 919) as a setup helper.

---

## Step 3: Update existing tests

### 3a. `test_toggle_tag_expansion_during_search_updates_filtered` (line 846)

This test currently asserts that after `ToggleTagExpansion`, the
filtered node's `is_expanded` matches the original. The assertion
doesn't change. The test verifies the same behavior.

**No code change needed.** The test still passes because the new
handler does the same in-place toggle.

### 3b. `test_toggle_tag_expansion_no_search_preserves_filtered` (line 872)

This test has no search query set. The current handler calls
`recompute_filtered_tag_nodes` (which returns `tag_tree_roots.clone()`
when query is empty). The new handler applies the toggle to
`filtered_tag_tree_roots` via `find_tag_node_mut`, which sets
`is_expanded` in-place. The assertion that both trees are expanded
still holds.

**No code change needed.**

### 3c. `test_tag_expansion_nonmatching_parent_matching_child` (line 897)

This test sets up a parent with a child whose label matches the search
("Jazz"). The filtered tree contains the parent (via child match) but
the parent is not expanded. Toggling expands both original and filtered.

The new handler toggles the parent in `tag_tree_roots` and then also
toggles it in `filtered_tag_tree_roots` (since "Parent" exists in both).
The assertion `assert!(app.filtered_tag_tree_roots[0].is_expanded)`
still passes.

**No code change needed.**

### 3d. `test_toggle_expansion_during_search_updates_filtered` (line 937)

Tests that `ToggleExpansion` expands both original and filtered nodes
during search. The new handler applies `restore_expansion_state` to
`filtered_root_nodes`, which sets `is_expanded` on the filtered node.
Assertions unchanged.

**No code change needed.**

### 3e. `test_toggle_expansion_no_search_preserves_filtered` (line 968)

No search active. The new handler applies `restore_expansion_state` to
`filtered_root_nodes` (the same walk applied to `root_nodes`), setting
`is_expanded` in-place. Assertions still pass.

**No code change needed.**

### 3f. Run full test suite

```sh
cargo test
cargo clippy
cargo fmt --check
```

---

## Step 4: Add a targeted test for expansion during search

Add a test that verifies the key invariant: **expanding/collapsing
during search does not alter the filtered tree's structure** (only
`is_expanded` flags change, and node/child counts are preserved).

### New test: `test_toggle_expansion_preserves_filtered_structure`

```rust
#[test]
fn test_toggle_expansion_preserves_filtered_structure() {
    let mut app = FileTreeApp::new(
        vec![],
        &["mp3"],
        PathBuf::from("/tmp/test.json"),
        None,
    );
    let dir = FileNode::new_directory(
        "Music".to_string(),
        PathBuf::from("/Music"),
        vec![
            FileNode::new_file(
                "rock.mp3".to_string(),
                PathBuf::from("/Music/rock.mp3"),
            ),
            FileNode::new_file(
                "jazz.mp3".to_string(),
                PathBuf::from("/Music/jazz.mp3"),
            ),
        ],
    );
    app.root_nodes = vec![Some(dir)];
    app.search_query = "rock".to_string();
    app.filtered_root_nodes = recompute_filtered_nodes(&app);

    // Capture the structure before toggle
    let child_count_before =
        app.filtered_root_nodes[0].as_ref().unwrap().children.len();
    let file_count_before =
        app.filtered_root_nodes[0].as_ref().unwrap().file_count;

    // Toggle expansion
    let _ = update(
        &mut app,
        Message::ToggleExpansion(PathBuf::from("/Music")),
    );

    // Structure must be unchanged — same children, same file count
    let filtered = app.filtered_root_nodes[0].as_ref().unwrap();
    assert_eq!(filtered.children.len(), child_count_before);
    assert_eq!(filtered.file_count, file_count_before);
    // Only expansion flag should have changed
    assert!(filtered.is_expanded);
}
```

### New test: `test_toggle_tag_expansion_preserves_filtered_structure`

Similar for tag tree.

```rust
#[test]
fn test_toggle_tag_expansion_preserves_filtered_structure() {
    let mut app = FileTreeApp::new(
        vec![],
        &["mp3"],
        PathBuf::from("/tmp/test.json"),
        None,
    );
    app.tag_tree_roots = vec![TagTreeNode {
        label: "Parent".to_string(),
        children: vec![
            TagTreeNode {
                label: "Rock".to_string(),
                children: vec![],
                file_paths: vec![PathBuf::from("/a.mp3")],
                is_expanded: false,
                file_count: 1,
            },
            TagTreeNode {
                label: "Jazz".to_string(),
                children: vec![],
                file_paths: vec![PathBuf::from("/b.mp3")],
                is_expanded: false,
                file_count: 1,
            },
        ],
        file_paths: vec![],
        is_expanded: false,
        file_count: 2,
    }];
    app.search_query = "Rock".to_string();
    app.filtered_tag_tree_roots = recompute_filtered_tag_nodes(&app);

    let child_count_before =
        app.filtered_tag_tree_roots[0].children.len();
    let file_count_before =
        app.filtered_tag_tree_roots[0].file_count;

    let path = vec!["Parent".to_string()];
    let _ = update(&mut app, Message::ToggleTagExpansion(path));

    // Structure must be unchanged
    assert_eq!(
        app.filtered_tag_tree_roots[0].children.len(),
        child_count_before
    );
    assert_eq!(
        app.filtered_tag_tree_roots[0].file_count,
        file_count_before
    );
    assert!(app.filtered_tag_tree_roots[0].is_expanded);
    // Matching child still present, non-matching child still absent
    assert_eq!(
        app.filtered_tag_tree_roots[0]
            .children
            .iter()
            .map(|c| c.label.as_str())
            .collect::<Vec<_>>(),
        vec!["Rock"]
    );
}
```

---

### Search-clear path

The `SearchCleared` and `SearchQueryChanged("")` handlers (lines
645–659) reassign `filtered_root_nodes = root_nodes.clone()` and
`filtered_tag_tree_roots = tag_tree_roots.clone()`. These paths are
unaffected by this change — the clone captures the current
`is_expanded` state from the original trees.

## Verification Checklist

- [ ] `cargo build` succeeds with no new warnings.
- [ ] `cargo test` — all existing tests pass.
- [ ] New structure-preservation tests pass.
- [ ] `cargo clippy` — no new lints.
- [ ] `cargo fmt --check` — no formatting regressions.
- [ ] Manual smoke test: load a large collection (~32K tracks), type a
  search query, expand/collapse directories and tag categories. Verify
  operations are instant and filtered view remains correct.
- [ ] Verify expand/collapse behavior is identical to before the change
  (expansion state persists across mode toggles, search clears, etc.).

---

## Risk Assessment

| Risk | Likelihood | Mitigation |
|---|---|---|
| Syncing `is_expanded` to filtered tree goes out of sync with unfiltered tree | Low | `expanded_dirs` is the single source of truth for file tree; `find_tag_node_mut` uses the same path both times for tag tree |
| Pruned nodes in filtered tree cause incorrect expansion display | Low | `restore_expansion_state` silently skips missing paths; `find_tag_node_mut` returns `None` for missing paths — both are correct no-ops |
| Future code adds a filter that depends on `is_expanded` | Low | Comment added in handler explains the invariant; could add a doc comment to `filter_file_node` / `filter_tag_node` noting they must not depend on `is_expanded` |
| `recompute_filtered_*` functions become dead code | Low | Annotated with `#[allow(dead_code)]`; test-only helpers kept in the module for test setup |

## Files Changed

| File | Change |
|---|---|
| `src/gui/update.rs` | Modify `ToggleExpansion` handler (~line 303) |
| `src/gui/update.rs` | Modify `ToggleTagExpansion` handler (~line 754) |
| `src/gui/update.rs` | Add two new tests (structure preservation) |
| `src/gui/update.rs` | Add `#[allow(dead_code)]` to `recompute_filtered_nodes` (test-only after change) |
| `src/gui/update.rs` | Add `#[allow(dead_code)]` to `recompute_filtered_tag_nodes` (test-only after change) |
