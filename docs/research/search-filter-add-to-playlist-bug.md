# Bug: Category "Add to right panel" ignores active text search filter

**Date**: 2026-07-06  
**Status**: Root cause identified, fix proposed

## Problem Summary

When a text search term is active in the left panel, the category nodes
(Directory, Genre, Creator) correctly display filtered file counts (number in
parentheses and blue highlight).  However, right-clicking a category and
selecting "Add all files to right panel" adds **all** files from that category
to the playlist, ignoring the active search filter. Only files matching the text
search should be added.

## Root Cause Analysis

### Data Flow During Search

1. When a search query is entered, `perform_search()` (state.rs:292) executes a
   Tantivy full-text search and stores the matching `PathBuf`s in
   `app.last_search_matches: Option<HashSet<PathBuf>>`.

2. The search results are then used to prune the original trees into two
   filtered tree copies:
   - `app.filtered_root_nodes: Vec<Option<FileNode>>` — file tree filtered by
     search
   - `app.filtered_tag_tree_roots: Vec<TagTreeNode>` — tag tree (Genre,
     Creator) filtered by search

3. The left panel UI uses these **filtered** trees when rendering:
   - `create_left_panel_file_tree_browser` (left_panel.rs:135) uses
     `app.filtered_root_nodes` when search is active
   - `create_left_panel_tag_tree_browser` (left_panel.rs:203) uses
     `app.filtered_tag_tree_roots` when search is active

4. The filtered trees correctly recalculate `file_count` to reflect only
   matching files, so the counts and blue highlights are correct.

### Where the Bug Is

Two message handlers in `src/gui/update.rs` ignore the search filter:

#### 1. `Message::AddTagNodeToRightPanel` (line ~755)

```rust
Message::AddTagNodeToRightPanel(path) => {
    app.right_panel_shuffled = false;
    if let Some(node) = find_tag_node_mut(&mut app.tag_tree_roots, &path) {
        let mut files = Vec::new();
        collect_tag_node_files(node, &mut files);
        // BUG: collected files are not filtered against
        // app.last_search_matches — all files are added unconditionally
        for file in files {
            // ... adds all files unconditionally
        }
    }
    Task::none()
}
```

> **Note:** The tree lookup in `app.tag_tree_roots` (unfiltered) is not a
> bug — `prune_tag_node` preserves node labels, so the same node is found
> by path in either tree. The bug is solely in the lack of filtering on the
> collected files.

#### 2. `Message::AddDirectoryToRightPanel` (line ~419)

```rust
Message::AddDirectoryToRightPanel(dir_path) => {
    app.right_panel_shuffled = false;
    for root in app.root_nodes.iter().flatten() {
        if let Some(node) = find_node_by_path(root, &dir_path) {
            let mut files = Vec::new();
            collect_files_recursively(node, &mut files);
            // BUG: collected files are not filtered against
            // app.last_search_matches — all files are added unconditionally
            // ... adds all files unconditionally
        }
    }
    Task::none()
}
```

> **Note:** As with the tag-tree handler, the tree lookup source is not the
> bug — directory paths are the same in filtered and unfiltered trees. The
> bug is the missing filter on the collected files.

### Why Filter Against `last_search_matches` Instead of Using Filtered Trees

Filtering collected files against `last_search_matches` is preferred over
switching the handlers to look up nodes in the filtered trees, even though
either approach would work with the current data structures:

- **Leaf nodes** in the filtered tree only survive `prune_tag_node` when at
  least one `file_path` matches, so their `file_paths` are already correct.
- **Non-leaf nodes** (the ones users right-click) have empty `file_paths` in
  practice — all files come from children, which are already pruned.

However, `last_search_matches` is the authoritative search result set from
Tantivy and does not depend on the internal structure of filtered trees. This
makes the fix more explicit, easier to audit, and resilient to future changes
in how filtered trees are computed.

## Proposed Fix

Filter collected files against `app.last_search_matches` before adding to the
right panel. This is the authoritative search result set from Tantivy.

### Changes to `src/gui/update.rs`

#### 1. Fix `Message::AddTagNodeToRightPanel`

After collecting files from the tag node, retain only those present in
`last_search_matches` when a search is active:

```rust
Message::AddTagNodeToRightPanel(path) => {
    app.right_panel_shuffled = false;
    if let Some(node) = find_tag_node_mut(&mut app.tag_tree_roots, &path) {
        let mut files = Vec::new();
        collect_tag_node_files(node, &mut files);

        // Filter files by active search, if any
        if let Some(ref matches) = app.last_search_matches {
            files.retain(|f| matches.contains(f));
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
}
```

#### 2. Fix `Message::AddDirectoryToRightPanel`

Same approach — filter collected files by search matches:

```rust
Message::AddDirectoryToRightPanel(dir_path) => {
    app.right_panel_shuffled = false;
    for root in app.root_nodes.iter().flatten() {
        if let Some(node) = find_node_by_path(root, &dir_path) {
            let mut files = Vec::new();
            collect_files_recursively(node, &mut files);

            // Filter files by active search, if any
            if let Some(ref matches) = app.last_search_matches {
                files.retain(|f| matches.contains(f));
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
}
```

### Why `last_search_matches` Is Safe to Use

- `last_search_matches` is set to `Some(matches)` by `perform_search()` every
  time a new search is run.
- It is set to `None` when the search query is cleared
  (`Message::SearchCleared` and `SearchQueryChanged("")`).
- When it is `Some(...)`, the search query is non-empty and the match set
  accurately reflects the files that pass the current search.
- The `Option` wrap already correctly gates the filter: no search → no
  filtering.

## Test Plan

1. **AddTagNodeToRightPanel with search active**: Create a test app with a tag
   tree containing, say, 5 tracks across 2 genres. Set a search query that
   matches 2 tracks. Call `Message::AddTagNodeToRightPanel` on the genre node.
   Assert only the 2 matching tracks are added to `right_panel_files`.

2. **AddTagNodeToRightPanel without search**: Same setup but no search query.
   Assert all 5 tracks are added (no regression).

3. **AddDirectoryToRightPanel with search active**: Similar test with a file
   tree. Search matches only some files in a directory. Assert only matching
   files are added.

4. **AddDirectoryToRightPanel without search**: Assert all files are added (no
   regression).

5. **AddTagNodeToRightPanel with empty search results**: Search is active but
   matches zero files (`last_search_matches = Some(HashSet::new())`). Calling
   `AddTagNodeToRightPanel` should add zero files (not fall through to adding
   everything).

6. **Single-track right-click unaffected**: `Message::AddToRightPanel` on a
   single track should still add just that track — this is unaffected since it
   doesn't use batch collection.

> **Test path note:** Test file paths may not exist on disk.
> `extract_media_metadata` returns default/empty metadata for non-existent
> paths. Tests validate only file-path membership in `right_panel_files`, not
> metadata correctness.

## Files to Modify

| File | Change |
|------|--------|
| `src/gui/update.rs` | Add `files.retain()` filter in `AddTagNodeToRightPanel` and `AddDirectoryToRightPanel` handlers |
| `src/gui/update.rs` (tests) | Add 5 test cases covering both handlers with/without search, plus empty-search edge case |

## Related Code Paths (for awareness, no changes needed)

| Path | Why Not Affected |
|------|-----------------|
| `Message::AddToRightPanel` (single track) | Adds a single explicit path — no batch collection |
| `Message::ExportRightPanelAsXspfTo` | Uses `displayed_right_panel_files()` which returns files already in the playlist |
| `filter_tag_node()` / `prune_tag_node()` | Used for display filtering only — not for data collection |
| Other batch-collection handlers | Only `AddDirectoryToRightPanel` and `AddTagNodeToRightPanel` use `collect_files_recursively` / `collect_tag_node_files` — no other handlers need changes |
