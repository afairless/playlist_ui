# Implementation Plan: Fix "Add to Right Panel" ignoring active text search filter

Source: `docs/research/search-filter-add-to-playlist-bug.md`

## Overview

When a text search term is active in the left panel, right-clicking a category
(Directory, Genre, Creator) and selecting "Add all files to right panel" adds
**all** files from that category, ignoring the active search filter. The fix is
to filter collected files against `app.last_search_matches` in both
`AddDirectoryToRightPanel` and `AddTagNodeToRightPanel` handlers.

## Branch

Work continues on `agent/fix-file-count-staleness` (with existing formatting
fixes committed first).

| # | Commit message | Logical unit | Key deliverables | Tests |
|---|---|---|---|---|
| 1 | `fix: add search filter to AddDirectoryToRightPanel handler` | AddDirectoryToRightPanel fix | `src/gui/update.rs` — add `files.retain()` filter against `app.last_search_matches` in the `AddDirectoryToRightPanel` handler | Unit (with search, without search) |
| 2 | `fix: add search filter to AddTagNodeToRightPanel handler` | AddTagNodeToRightPanel fix | `src/gui/update.rs` — add `files.retain()` filter against `app.last_search_matches` in the `AddTagNodeToRightPanel` handler | Unit (with search, without search, empty search results) |

## Details

### Step 1 — AddDirectoryToRightPanel fix

After `collect_files_recursively(node, &mut files)` in the
`AddDirectoryToRightPanel` handler, insert:

```rust
// Filter files by active search, if any
if let Some(ref matches) = app.last_search_matches {
    files.retain(|f| matches.contains(f));
}
```

**Test cases:**

1. **With search active** — Set up a file tree with 5 files, set a search query
   matching 2 files. Call `AddDirectoryToRightPanel`. Assert only 2 matching
   files appear in `right_panel_files`.
2. **Without search** — Same tree, no search query. Call
   `AddDirectoryToRightPanel`. Assert all 5 files appear (no regression).

### Step 2 — AddTagNodeToRightPanel fix

After `collect_tag_node_files(node, &mut files)` in the
`AddTagNodeToRightPanel` handler, insert:

```rust
// Filter files by active search, if any
if let Some(ref matches) = app.last_search_matches {
    files.retain(|f| matches.contains(f));
}
```

**Test cases:**

1. **With search active** — Set up a tag tree with 5 tracks across 2 genres,
   set a search query matching 2 tracks. Call `AddTagNodeToRightPanel`.
   Assert only 2 matching tracks are added.
2. **Without search** — Same setup, no search query. Assert all 5 tracks are
   added (no regression).
3. **Empty search results** — Search is active but matches zero files
   (`last_search_matches = Some(HashSet::new())`). Assert zero files added
   (not fall-through to adding everything).
