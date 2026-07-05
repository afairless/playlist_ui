# Tantivy Text Search Integration Research

## Decision Summary

Based on `search-performance-research.md`, we have settled on:

- **Search engine:** tantivy with minimal features
  (`default-features = false`)
- **Execution model:** Synchronous (no background thread, no async runtime)
- **Input handling:** Search is processed synchronously on every keystroke.
  Because tantivy queries complete in <1ms for typical queries and <10ms
  even in worst-case All-mode scans of 32k documents, no debounce is
  necessary — the UI remains responsive without delayed computation.
- **Search target:** The left panel (file tree + tag tree), which
  displays all ~32,000 music tracks. The right panel playlist reuses
  the same search result set (`last_search_matches`).
- **Search semantics:** Token-based `PhrasePrefixQuery` (with fuzzy
  fallback for short queries) replaces the current arbitrary-substring
  `.contains()` matching.
- **All open questions settled:** Tokenizer behavior, All-mode query
  construction, right panel search, and stored-path deserialization
  performance have all been empirically resolved.

This document explores the architecture for integrating tantivy with the
left panel, including how the index replaces the current metadata-reads-
from-disk approach, and how text search interacts with other left panel
controls.

## Current Architecture — Left Panel Search

### Three Trees

The left panel has three views, selected by the "Select by" button:

| Mode | Data source | Filter function | Search cost |
|---|---|---|---|
| Directory | `root_nodes` — `Vec<Option<FileNode>>` | `filter_file_node` | **32k disk reads** (`extract_media_metadata` per file) |
| GenreTag | `tag_tree_roots` — `Vec<TagTreeNode>` | `filter_tag_node` | In-memory string matching only |
| CreatorTag | `tag_tree_roots` — `Vec<TagTreeNode>` | `filter_tag_node` | In-memory string matching only |

### The Real Bottleneck

The `filter_file_node` function calls `file_matches_mode`, which calls
`extract_media_metadata(path)` for every file node in metadata search modes
(Creator/Album/Title/Genre/All). This reads each audio file's tag headers
(ID3, Vorbis Comments, etc.) from **disk** using the `lofty` crate.

Typing "rock" in All mode triggers **32,000 disk reads** of audio files.
This is the dominant cause of the freeze — far more than string allocations.

### Tag Tree Is Already In-Memory

`build_genre_tag_tree` / `build_creator_tag_tree` pre-extract metadata into
`TagTreeNode` labels and `file_paths` at startup. So `filter_tag_node` does
purely in-memory string matching on pre-loaded data — no disk I/O during
search. Tantivy's benefit here is smaller but still positive (set-lookup
replaces linear path scans).

## Proposed Architecture

### Tantivy Index Lifecycle

A single in-memory tantivy index is built once and rebuilt when the set of
visible files changes:

```text
App startup / Extension toggle / Directory added or removed
  -> scan_directory (builds file tree, as today)
  -> build_tantivy_index (new function)      # blocks briefly (200-400ms)
  -> build_genre_tag_tree / build_creator_tag_tree (as today)
```

The index is stored as a new field on `FileTreeApp`:

```rust,ignore
#[serde(skip)]
pub tantivy_index: Option<TantivyIndexWrapper>,
```

where `TantivyIndexWrapper` wraps an in-memory tantivy index + reader.

**New model fields:**

| Field | Type | Purpose |
|---|---|---|
| `tantivy_index` | `Option<TantivyIndexWrapper>` | In-memory tantivy index + reader |
| `search_generation` | `u64` | Monotonically incrementing counter; discards stale results |
| `last_search_matches` | `Option<HashSet<PathBuf>>` | Cached result from the most recent search; enables re-pruning when switching select modes without re-querying tantivy |

All three fields are `#[serde(skip)]`.

### New message variants

| Variant | Payload | Purpose |
|---|---|---|
| `PerformSearch` | `u64` | Runs the tantivy query, prunes both trees, and filters right panel files; the `u64` payload is the generation counter |

### `TantivyIndexWrapper` API

```rust,ignore
/// Wraps an in-memory tantivy index + reader.
///
/// TantivyIndexWrapper is NOT Clone - tantivy::IndexReader does not
/// implement Clone. If you need a second handle to the same index,
/// create a new reader from `index.reader()`. The struct is Send + Sync.
pub(crate) struct TantivyIndexWrapper {
    index: tantivy::Index,
    reader: tantivy::IndexReader,
    schema: tantivy::schema::Schema,
}

impl std::fmt::Debug for TantivyIndexWrapper {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TantivyIndexWrapper")
            .field("num_docs", &self.reader.searcher().num_docs())
            .field("schema", &self.schema)
            .finish_non_exhaustive()
    }
}

impl TantivyIndexWrapper {
    /// Search the index with the given query string and mode.
    /// Returns the set of matching file paths.
    ///
    /// Returns an error when the query could not be parsed or executed
    /// (invalid regex, tantivy internal error). The caller should log
    /// the error and return an empty set.
    pub(crate) fn search(
        &self,
        query: &str,
        mode: TextSearchMode,
    ) -> Result<HashSet<PathBuf>, tantivy::TantivyError>;
}
```

**Fallback when the index is `None`:** If `tantivy_index` is `None` when
a search fires (e.g., before the first index build completes), fall back
to the current linear-scan functions. This is a graceful degradation path.

### Index Schema

| Tantivy field | Source | Indexed | Stored | Purpose |
|---|---|---|---|---|
| `path` | `path.to_string_lossy()` | STRING | Yes | Identify matches; prune trees |
| `filename` | `path.file_name()` | TEXT | No | `TrackFilename` mode |
| `creator` | `extract_media_metadata` | TEXT | No | `Creator` / `All` mode |
| `album` | `extract_media_metadata` | TEXT | No | `Album` / `All` mode |
| `title` | `extract_media_metadata` | TEXT | No | `Title` / `All` mode |
| `genre` | `extract_media_metadata` | TEXT | No | `Genre` / `All` mode |

Note: `path` is `STRING` (raw, not tokenized) for `RegexQuery` support.
All metadata fields are `TEXT` (tokenized) for word-level and prefix
matching via `PhrasePrefixQuery`.

### Index Construction

Estimated time: ~200-400ms for 32k tracks (one-time, replaces per-search
disk reads). This runs synchronously and briefly blocks the UI. Because
it only happens at startup and on user-initiated changes (where a brief
pause is acceptable), no async loading state is needed.

Note: `build_genre_tag_tree` and `build_creator_tag_tree` do the same file
walk + metadata extraction. This is redundant. A future optimisation could
share the metadata between all three structures, but the one-time ~400ms
cost is acceptable given it replaces per-search 32k disk reads. Following
the three-stage data-pipeline model (ingestion -> transformation ->
output), the index build mixes ingestion (reading metadata from disk) and
transformation (converting to tantivy documents). This is acknowledged
technical debt acceptable for the initial implementation.

### Search Flow

**Before (current):**

```text
SearchQueryChanged(query)
  -> filter_file_node(tree)      # recursive, disk reads per file
  -> filter_tag_node(tag_tree)   # recursive, linear string scan
```

**After (proposed) - synchronous, no debounce:**

```text
SearchQueryChanged(query) / ToggleSearchMode / SearchCleared
  -> perform_search() (synchronous)
  -> tantivy_index.search(query)  # <1ms -> HashSet<PathBuf>
  -> prune_file_tree(tree, matches)  # recursive, but set lookup
  -> prune_tag_tree(tag_tree, matches)  # recursive, but set lookup
```

The three recompute functions change name/semantics:

| Current | Proposed | What changed |
|---|---|---|
| `recompute_filtered_nodes` | `prune_file_tree` | No more disk reads. Uses set lookup + query string for directory-name matching. |
| `recompute_filtered_tag_nodes` | `prune_tag_tree` | Replaces linear path scans with set lookup. No label matching needed. |
| `recompute_filtered_right_panel_files` | Inline in `perform_search` | Filters right panel files against `last_search_matches` HashSet. |

### Search Processing (Synchronous)

Because tantivy queries complete in <1ms for typical queries and <10ms even
in worst-case All-mode scans, the search is processed synchronously on
every keystroke with no debounce delay. This is simpler than the current
code (which also processes synchronously, but with 32k disk reads causing
second-long freezes).

The generation counter is retained as a safety net for the rare case where
a `ToggleExtension` or `AddDirectory` mutation interleaves during search
processing. In practice, since iced processes messages sequentially, this
can only happen if a message handler re-enters the update loop.

**`SearchQueryChanged` handler:**

```rust,ignore
Message::SearchQueryChanged(query) => {
    app.search_query = query;
    if query.is_empty() {
        // Restore full unfiltered trees
        app.filtered_root_nodes = app.root_nodes.clone();
        app.filtered_tag_tree_roots = app.tag_tree_roots.clone();
        app.filtered_right_panel_files = Vec::new();
        app.last_search_matches = None;
    } else {
        app.perform_search();
    }
    Task::none()
}
```

**`perform_search()` helper method on `FileTreeApp`:**

```rust,ignore
impl FileTreeApp {
    fn perform_search(&mut self) {
        self.search_generation += 1;
        let gen = self.search_generation;

        let matches = if let Some(ref index) = self.tantivy_index {
            match index.search(&self.search_query, self.search_mode) {
                Ok(m) => m,
                Err(e) => {
                    log::error!("Tantivy search failed: {e}");
                    HashSet::new()
                },
            }
        } else {
            HashSet::new()
        };

        // Guard: discard stale results if a newer mutation occurred.
        if gen != self.search_generation { return; }

        self.last_search_matches = Some(matches.clone());

        // Prune both left panel trees against the match set
        self.filtered_root_nodes = self
            .root_nodes
            .iter()
            .map(|node_opt| {
                node_opt.as_ref().and_then(|n| {
                    prune_file_tree(n, &matches, &self.search_query,
                                    self.search_mode)
                })
            })
            .collect();

        self.filtered_tag_tree_roots = self
            .tag_tree_roots
            .iter()
            .filter_map(|n| prune_tag_node(n, &matches))
            .collect();

        // Filter right panel playlist via HashSet lookup
        self.filtered_right_panel_files = self
            .right_panel_files
            .iter()
            .filter(|f| matches.contains(&f.path))
            .cloned()
            .collect();
    }
}
```

### Message Handlers

**`ToggleSearchMode`:**

```rust,ignore
Message::ToggleSearchMode => {
    app.search_mode = match app.search_mode {
        TextSearchMode::All => TextSearchMode::DirectoryPath,
        TextSearchMode::DirectoryPath => TextSearchMode::TrackFilename,
        TextSearchMode::TrackFilename => TextSearchMode::Creator,
        TextSearchMode::Creator => TextSearchMode::Album,
        TextSearchMode::Album => TextSearchMode::Title,
        TextSearchMode::Title => TextSearchMode::Genre,
        TextSearchMode::Genre => TextSearchMode::All,
    };
    if !app.search_query.is_empty() {
        app.perform_search();
    }
    Task::none()
}
```

**`SearchCleared`:**

```rust,ignore
Message::SearchCleared => {
    app.search_query = String::new();
    app.last_search_matches = None;
    app.filtered_root_nodes = app.root_nodes.clone();
    app.filtered_tag_tree_roots = app.tag_tree_roots.clone();
    app.filtered_right_panel_files = Vec::new();
    Task::none()
}
```

**`ToggleLeftPanelSelectMode` (also fixes pre-existing bug where switching
modes left stale filtered_tag_tree_roots):**

```rust,ignore
Message::ToggleLeftPanelSelectMode => {
    // ... existing logic to rebuild app.tag_tree_roots ...
    app.left_panel_selection_mode = new_mode;

    if !app.search_query.is_empty() {
        if let Some(ref matches) = app.last_search_matches.clone() {
            app.filtered_root_nodes = app
                .root_nodes
                .iter()
                .map(|node_opt| {
                    node_opt.as_ref().and_then(|n| {
                        prune_file_tree(n, matches, &app.search_query,
                                        app.search_mode)
                    })
                })
                .collect();
            app.filtered_tag_tree_roots = app
                .tag_tree_roots
                .iter()
                .filter_map(|n| prune_tag_node(n, matches))
                .collect();
        } else {
            app.perform_search();
        }
    }
    Task::none()
}
```

**`ToggleExtension`:**

```rust,ignore
Message::ToggleExtension(ext) => {
    // ... existing logic to update selected_extensions ...
    app.root_nodes = app
        .top_dirs
        .iter()
        .map(|dir| scan_directory(dir, ...))
        .collect();
    app.tantivy_index =
        Some(build_tantivy_index(&app.root_nodes));
    if !app.search_query.is_empty() {
        app.perform_search();
    }
    // Rebuild tag trees against the new file set
    app.tag_tree_roots = build_genre_tag_tree(
        &app.top_dirs, &app.selected_extensions);
    if !app.search_query.is_empty() {
        app.perform_search();
    }
    Task::none()
}
```

`AddDirectory`, `DirectoryAdded`, and `RemoveTopDir` follow the same
pattern: rebuild the file tree, rebuild the tantivy index, re-apply
search filter if active.

### Pruning Functions

`prune_file_tree` and `prune_tag_node` live in `update.rs` (or a new
`src/gui/tantivy_search.rs` module):

```rust,ignore
/// Recursively prune a FileNode tree, keeping only nodes whose subtree
/// contains at least one path in `matches`.
///
/// Unlike the current `filter_file_node`, this function does **no**
/// metadata extraction from disk. However, it preserves the current
/// behaviour of keeping a directory node when its name or path matches
/// the query string, even when no file children match. This is done by
/// checking the directory's own path and name against the query in
/// DirectoryPath and All modes.
fn prune_file_tree(
    node: &FileNode,
    matches: &HashSet<PathBuf>,
    query: &str,
    mode: TextSearchMode,
) -> Option<FileNode>;

/// Recursively prune a TagTreeNode tree, keeping only nodes whose
/// subtree contains at least one path in `matches`.
fn prune_tag_node(
    node: &TagTreeNode,
    matches: &HashSet<PathBuf>,
) -> Option<TagTreeNode>;
```

Key difference: these functions do **no metadata extraction from disk**.
They only check `HashSet::contains` against the pre-computed set of
matching paths. This eliminates the O(n) disk-read bottleneck.

**Directory-name matching in `prune_file_tree`:** The current
`filter_file_node` keeps a directory node if its name or path matches the
query, even when no files inside match. The proposed `prune_file_tree`
preserves this: before recursing into children, it checks whether the
directory's own path (in `DirectoryPath` mode) or name (in `All` mode)
matches the query string. This is a simple string comparison, not a disk
read, so it adds negligible cost.

When a node is kept, its `file_count` is recomputed to reflect only the
visible (matching) descendants.

### Tag Tree Pruning

`prune_tag_node` operates purely on the `HashSet<PathBuf>` from the
tantivy query. No label matching, no path string scans. A single tantivy
query determines all visible nodes, and both trees prune against the same
result set.

**Why label matching is no longer needed.** The current `filter_tag_node`
uses substring matching on node labels as a workaround for not having
per-file metadata readily available. This is semantically loose:
searching in Album mode while viewing the Creator tag tree matches
creator names instead of album names. With tantivy, the query is
field-aware, giving consistent semantics regardless of which tree is
displayed.

**Tokenization makes partial-word matches natural.** A concern with
dropping label matching is that searching for "rock" should still match
"Progressive Rock". tantivy's `SimpleTokenizer` splits on whitespace
and lowercases, so `"Progressive Rock"` is tokenised as
`["progressive", "rock"]`. A `TermQuery` for `"rock"` on the genre
field matches the `"rock"` token. The same applies to artist names
("The Beatles" -> `["the", "beatles"]`, query `"beatles"` matches),
album names, and track titles.

**Handling empty `file_paths` on intermediate nodes.** The tag tree
hierarchy (Genre -> Artist -> Album -> Track) stores `file_paths` only
on leaf (track) nodes. Intermediate nodes have `file_paths: vec![]`.
The pruning function handles this by recursion: intermediate nodes
survive if any descendant leaf matches.

### The Tantivy Search Query

The synchronous `perform_search()` method builds a query that respects
the current `TextSearchMode` field restriction:

```text
Match search_mode:
    All          -> multi-field: creator|album|title|genre|filename|path
    Creator      -> single-field: creator
    Album        -> single-field: album
    Title        -> single-field: title
    Genre        -> single-field: genre
    DirectoryPath -> single-field: path
    TrackFilename -> single-field: filename
```

**Key design rule:** The search mode field restriction is enforced by
tantivy, not by the tree pruning logic. The resulting
`HashSet<PathBuf>` contains only files where the specified field(s)
match the query. The tree pruning code is purely a visual concern.

**Right panel integration.** The same `last_search_matches` HashSet
also filters the right panel playlist. `Filtered_right_panel_files` is
computed as `right_panel_files.iter().filter(|f| matches.contains(&f.path))`

- a single HashSet lookup per entry.

### Text Search Strategies

**Field types.** Metadata fields use `TEXT` (tokenized). The file path
is `STRING` (raw, not tokenized). A separate `filename` field is `TEXT`.

**Tokenizer.** `SimpleTokenizer` (whitespace split) + `LowerCaser`
(lowercase). No stop-word removal - "the" in "The Beatles" is indexed
normally.

**Query strategy: `PhrasePrefixQuery` (Recommended).** Matches phrases
where the last term is a prefix.

| User types | Query built | Matches |
|---|---|---|
| `rock` | `PhrasePrefixQuery(["rock"])` | "Rock", "Progressive Rock" |
| `prog` | `PhrasePrefixQuery(["prog"])` | "Progressive" |
| `miles dav` | `PhrasePrefixQuery(["miles", "dav"])` | "Miles Davis" |
| `dark side` | `PhrasePrefixQuery(["dark", "side"])` | "The Dark Side of the Moon" |

**Path searches use `RegexQuery`.** Since the path field is `STRING`,
substring matching uses `RegexQuery` with patterns like `.*beatles.*`.

**Fuzzy fallback for short queries.** When `PhrasePrefixQuery` returns
zero results and the query is <= 3 characters, retry with
`FuzzyTermQuery` (distance 1). Handles typos like `"ruock"` -> `"rock"`.

**All Mode Query Construction.** `All` mode builds a `BooleanQuery`
with `Occur::Should` (OR) across all six fields. Each TEXT field uses
`PhrasePrefixQuery`; the STRING path field uses `RegexQuery`.

**What is lost vs. current `.contains()` behaviour.** The current
filter uses case-insensitive substring matching. A query for `"ive"`
matches `"Progressive"`. With token-based search, `"ive"` does not
match - they are different tokens. Token-prefix matching covers the
common cases, and arbitrary substring matching is extremely rare in
music searches.

### Right Panel Search

The right panel search does **not** need its own tantivy index. After
`perform_search()` produces `last_search_matches`, the right panel
filter becomes a single `HashSet` lookup per entry - trivially fast.

This eliminates `recompute_filtered_right_panel_files` as a standalone
function; the filter is a 5-line block inside `perform_search()`.

## Text Search x Left Panel Controls

### File Extensions (AND filter)

The "File Extensions" toggle and text search act as AND. When extensions
change, the tantivy index is rebuilt to include only files matching the
selected extensions, then the search filter is re-applied.

### "Select by" (Rearranges the view, not a filter)

Switching between Directory/GenreTag/CreatorTag rearranges the view
without filtering. When a search is active, both `prune_file_tree` and
`prune_tag_tree` are re-run against the cached match set.

**Current code has a bug here:** `ToggleLeftPanelSelectMode` builds the
new `tag_tree_roots` but does NOT re-run the tag tree filter, leaving
stale `filtered_tag_tree_roots`. The proposed fix (shown in the handler
above) re-prunes against `last_search_matches` after rebuilding the tag
tree.

### "Sort" (Rearranges within the tree, not a filter)

No changes needed. The sort mode parameter flows through the rendering
pipeline unchanged, operating on whichever tree is displayed (filtered
or unfiltered).

## Dependencies

Add to `Cargo.toml`:

```toml
tantivy = { version = "0.23" }
```

Use tantivy's default features (no mmap required - we use
`Index::create_in_ram()`). `TermQuery`, `PhrasePrefixQuery`,
`FuzzyTermQuery`, `RegexQuery`, and `BooleanQuery` are available
without feature flags. `SimpleTokenizer`, `LowerCaser`, and
`TextAnalyzer` are part of the core crate.

Adds ~30-60s to clean builds, and increases the release binary by an
estimated ~4-8 MB (tantivy's core alone is ~3-4 MB stripped).

## Testing

### Unit Tests

**`build_tantivy_index`**

| Test | Input | Expected |
|---|---|---|
| `test_build_empty` | `root_nodes = []` | Empty index, search returns empty set |
| `test_build_single_file` | One `FileNode::File` with metadata | Index contains 1 doc; search by title returns the path |
| `test_build_many_files` | 100+ files with varied metadata | All 100 docs retrievable; multi-field All-mode works |
| `test_build_skips_non_audio` | Mix of audio and non-audio file nodes | Only audio files indexed |

**`prune_file_tree`**

| Test | Input | Expected |
|---|---|---|
| `test_prune_empty_tree` | `None` node, any matches | Returns `None` |
| `test_prune_no_matches` | 5-file tree, empty `HashSet` | Returns `None` for root |
| `test_prune_all_match` | 3-file tree, all paths in set | All 3 files preserved |
| `test_prune_partial` | 3-file tree, 1 path in set | Only the matching file subtree preserved |
| `test_prune_file_count_updated` | Directory with 3 children, 1 match | Filtered directory shows `file_count = 1` |

**`prune_tag_node`**

| Test | Input | Expected |
|---|---|---|
| `test_prune_tag_empty` | Empty `HashSet` | Returns `None` |
| `test_prune_tag_all_match` | All leaf paths in set | Full tree preserved |
| `test_prune_tag_partial` | Genre -> Artist -> Album -> Track, only 1 match | Only the matching chain preserved |
| `test_prune_tag_file_count_updated` | Genre with 42 tracks, 3 match | Filtered genre shows `file_count = 3` |

**Generation counter (safety net)**

| Test | Input | Expected |
|---|---|---|
| `test_generation_stale_result_discarded` | Call with stale gen counter | No state change |
| `test_generation_latest_result_applied` | Call with current gen counter | Trees pruned |

### Integration Tests

| Test | What it verifies |
|---|---|
| `test_full_search_flow` | Build index with known files -> query -> verify both pruned trees contain expected paths |
| `test_index_rebuild_on_extension_toggle` | Toggle extension -> index rebuilt -> search on new extension set works |
| `test_select_mode_switch_preserves_search` | Search in Directory mode -> switch to GenreTag -> filtered tag tree still shows matching tracks |
| `test_toggle_left_panel_select_mode_during_search` | Switch between Dir/Genre/Creator while search is active -> each mode shows correctly filtered results |

### Property-Based Tests

| Test | Invariant |
|---|---|
| `test_index_search_roundtrip` | For any file with metadata V, searching for V in the corresponding mode returns that file |
| `test_index_search_roundtrip_unicode` | Same with unicode metadata (artists like "M\u00f8", "Bj\u00f6rk") to verify unicode tokenization |
| `test_prune_idempotency` | Pruning an already-pruned tree produces the same result |
| `test_fuzzy_fallback_typo` | Query "ruock" with 0 direct results triggers fuzzy fallback and returns "rock" (distance 1) |
| `test_fuzzy_fallback_boundary_4chars` | Query "rockk" (4 chars) with 0 direct results does NOT trigger fuzzy fallback |
| `test_fuzzy_fallback_no_false_positive` | Short query with no close match returns empty even after fuzzy fallback |

## Trade-Offs

| Aspect | Benefit | Cost |
|---|---|---|
| **Search speed** | O(1) index lookup replaces O(n) disk reads per search | 200-400ms one-time index rebuild on startup and extension/directory changes |
| **Memory** | Fast in-memory queries | Tantivy index overhead: ~10-30 MB for 32k documents with 6 text fields |
| **Compile time** | In-memory index, no mmap | Adds ~30-60s to clean builds (tantivy is large) |
| **Binary size** | - | Estimated ~4-8 MB increase. Affects dev iteration time (slower linking) |
| **Code complexity** | Pruning functions simpler than current filter functions | Schema + query construction adds new abstraction. `TantivyIndexWrapper` needs manual `Debug`; is not `Clone`. |

## Performance Validation

### Stored Field Deserialization (Settled)

**Decision: Stored path deserialization is not a bottleneck.**

Empirical benchmarking at 32k documents in release mode shows all
scenarios well under the ~16ms frame budget:

| Result set size | Example query | Total time |
|---|---|---|
| 2,000 | `creator:"pink floyd"` | 0.80ms |
| 4,000 | `genre:rock` | 1.20ms |
| 8,000 | `genre:rock OR genre:jazz` | 3.27ms |
| 12,000 | 3 genres OR'd | 4.65ms |
| 20,000 | 5 genres OR'd | 9.82ms |
| 32,000 | All docs | 9.97ms |

No special optimization is needed beyond the baseline design.

## Caveats and Known Limitations

### Behavioural Changes from Current Code

1. **Arbitrary substring matching is lost.** The current code uses
   case-insensitive `.contains()` - "ive" matches "Progressive".
   tantivy's token-based search splits on word boundaries, so "ive"
   no longer matches. This is acceptable for music search patterns.

2. **Directory-name matching is preserved** in `prune_file_tree` via
   a lightweight string comparison on the directory's path/name.

3. **Search mode semantics become consistent across trees.** Currently,
   searching in Album mode while viewing the Creator tag tree matches
   creator names. With tantivy, the field-aware query always searches
   the correct field.

### Tantivy Compilation Overhead

- Clean build: +30-60s
- Incremental builds after tantivy changes: +5-15s
- Release binary: +4-8 MB

### TantivyIndexWrapper is not Clone

If the model ever needs `Clone` (e.g., for iced subscriptions), the
wrapper must provide a `clone_with_new_reader()` method or the index
must be rebuilt. Currently, the model does not need `Clone`.

## Next Steps

1. **Proceed to implementation.** All open questions have been settled.
2. **Create a TODO.md** implementation plan using the `write-todo-from-plan`
   skill, breaking this design into small, testable, commit-sized steps.
3. **Implement incrementally** following the `incremental-development` skill:
   one step at a time, with tests passing and a conventional commit at each
   step.
