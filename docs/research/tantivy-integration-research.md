# Tantivy Text Search Integration Research

## Decision Summary

Based on `search-performance-research.md`, we have settled on:

- **Search engine:** tantivy with minimal features
  (`default-features = false`)
- **Execution model:** Synchronous (no background thread for now)
- **Input handling:** Debounce keystrokes before firing the search
- **Search target:** The left panel (file tree + tag tree), which
  displays all ~32,000 music tracks

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

```
App startup / Extension toggle / Directory added or removed
  → scan_directory (builds file tree, as today)
  → build_tantivy_index (new function)
  → build_genre_tag_tree / build_creator_tag_tree (as today)
```

The index is stored as a new field on `FileTreeApp`:

```rust
#[serde(skip)]
pub tantivy_index: Option<TantivyIndexWrapper>,
```

where `TantivyIndexWrapper` wraps an in-memory tantivy index + reader.

**New model fields:**

| Field | Type | Purpose |
|---|---|---|
| `tantivy_index` | `Option<TantivyIndexWrapper>` | In-memory tantivy index + reader |
| `search_generation` | `u64` | Monotonically incrementing counter for debounce cancellation |

**New message variants:**

| Variant | Purpose |
|---|---|
| `PerformSearch(u64)` | Runs the tantivy query and prunes trees; `u64` is the generation |

**`TantivyIndexWrapper` API:**

```rust
/// Wraps an in-memory tantivy index + reader. Must be Send + Sync
/// (tantivy's Index and IndexReader both implement these traits).
pub(crate) struct TantivyIndexWrapper {
    index: tantivy::Index,
    reader: tantivy::IndexReader,
    schema: tantivy::schema::Schema,
}

impl TantivyIndexWrapper {
    /// Search the index with the given query string and mode.
    /// Returns the set of matching file paths.
    /// If the query is empty, returns an empty set (caller should
    /// short-circuit and show the full unfiltered tree).
    pub(crate) fn search(
        &self,
        query: &str,
        mode: TextSearchMode,
    ) -> HashSet<PathBuf>;
}
```

**Fallback when the index is `None`:** If `tantivy_index` is `None` when
`PerformSearch` fires (e.g., before the first index build completes), fall
back to the current linear-scan `recompute_filtered_nodes` and
`recompute_filtered_tag_nodes` functions. This is a graceful degradation
path that ensures the search bar is never broken, even during startup.

### Index Schema

| Tantivy field | Source | Indexed | Stored | Purpose |
|---|---|---|---|---|
| `path` | `path.to_string_lossy()` | text | Yes | Identify matches; prune trees |
| `filename` | `path.file_name()` | text | No | `TrackFilename` mode |
| `creator` | `extract_media_metadata` | text | No | `Creator` / `All` mode |
| `album` | `extract_media_metadata` | text | No | `Album` / `All` mode |
| `title` | `extract_media_metadata` | text | No | `Title` / `All` mode |
| `genre` | `extract_media_metadata` | text | No | `Genre` / `All` mode |

Each of the ~32,000 files becomes one tantivy document.

### Index Construction

```
fn build_tantivy_index(root_nodes: &[Option<FileNode>]) -> TantivyIndexWrapper
    1. Create in-memory tantivy index (no mmap, no compression)
    2. Walk the FileNode tree recursively
    3. For each FileNode::File:
       a. Read metadata via extract_media_metadata (one-time cost)
       b. Add document to index
    4. Commit and return reader

Estimated time: ~200–400ms for 32k tracks (one-time, replaces per-search
disk reads).
```

Note: `build_genre_tag_tree` and `build_creator_tag_tree` do the same file
walk + metadata extraction. This is redundant. A future optimisation could
share the metadata between all three structures, but the one-time ~400ms
cost is acceptable given it replaces per-search 32k disk reads.

### Search Flow

**Before (current):**

```
SearchQueryChanged(query)
  → filter_file_node(tree)      # recursive, disk reads per file
  → filter_tag_node(tag_tree)   # recursive, linear string scan
```

**After (proposed):**

```
SearchQueryChanged(query)
  [debounce 150ms → PerformSearch]
  → tantivy_index.search(query)  # < 1ms → HashSet<PathBuf>
  → prune_file_tree(tree, matches)  # recursive, but set lookup
  → prune_tag_tree(tag_tree, matches)  # recursive, but set lookup
```

The three recompute functions change name/semantics:

| Current | Proposed | What changed |
|---|---|---|
| `recompute_filtered_nodes` | `prune_file_tree` | No more `file_matches_mode` / disk reads. Uses set lookup against tantivy results. |
| `recompute_filtered_tag_nodes` | `prune_tag_tree` | Replaces linear path scans with set lookup. Label matching still works the same way. |
| `recompute_filtered_right_panel_files` | **TBD** (see Open Question #2) | Right panel filtering may also use tantivy, or keep its current linear scan. |

### Debounce Mechanism

The debounce uses a generation-counter pattern within iced's architecture.
`SearchQueryChanged` stores the query immediately (for UI responsiveness)
and spawns a delayed `PerformSearch` task. Only the most recent generation's
results are applied:

```rust
Message::SearchQueryChanged(query) => {
    app.search_query = query;
    app.search_generation += 1;
    let gen = app.search_generation;
    Task::perform(
        async move {
            iced::time::sleep(std::time::Duration::from_millis(150)).await;
            gen
        },
        Message::PerformSearch,
    )
}
Message::PerformSearch(gen) => {
    if gen != app.search_generation { return Task::none(); }
    // ... run tantivy query, prune trees ...
}
```

### Pruning Functions

`prune_file_tree` and `prune_tag_tree` live in `update.rs` (or a new
`src/gui/tantivy_search.rs` module) and operate on the tantivy result set:

```rust
/// Recursively prune a FileNode tree, keeping only nodes whose subtree
/// contains at least one path in `matches`.
fn prune_file_tree(
    node: &FileNode,
    matches: &HashSet<PathBuf>,
) -> Option<FileNode>;

/// Recursively prune a TagTreeNode tree, keeping only nodes whose subtree
/// contains at least one path in `matches`.
fn prune_tag_node(
    node: &TagTreeNode,
    matches: &HashSet<PathBuf>,
) -> Option<TagTreeNode>;
```

Key difference from the current `filter_file_node` / `filter_tag_node`:
these functions do **no metadata extraction and no string matching** — they
only check `HashSet::contains` against the pre-computed set of matching
paths. This eliminates the O(n) disk-read bottleneck.

When a node is kept, its `file_count` is recomputed to reflect only the
visible (matching) descendants, not the original total. This provides
accurate counts in the filtered view.

### Tag Tree Pruning

The `prune_tag_node` function also operates purely on the
`HashSet<PathBuf>` from the tantivy query — no label matching, no path
string scans. A single tantivy query determines all visible nodes, and
both trees prune against the same result set.

**Why label matching is no longer needed.** The current
`filter_tag_node` uses substring matching on node labels (genre names,
artist names, album names, track titles) as a workaround for not having
per-file metadata readily available during filtering. This is fast
(in-memory, no disk I/O) but semantically loose: searching in Album
mode while viewing the Creator tag tree matches against creator names
instead of album names.

With tantivy, the query is field-aware — `Genre` mode searches the
`genre` field, `Creator` mode searches the `creator` field, etc. The
tag tree then shows only nodes whose tracks appear in the tantivy
result set. This gives consistent semantics regardless of which tag
tree is displayed.

**Tokenization makes partial-word matches natural.** A concern with
dropping label matching is that searching for "rock" should still match
"Progressive Rock". tantivy's `SimpleTokenizer` splits on whitespace
and lowercases, so `"Progressive Rock"` is tokenised as
`["progressive", "rock"]`. A `TermQuery` for `"rock"` on the genre
field matches the `"rock"` token — no label matching required. The
same applies to artist names ("The Beatles" → `["the", "beatles"]`,
query `"beatles"` matches), album names, and track titles.

For prefix queries (e.g., "Prog" → "Progressive Rock"), see the
**Text Search Strategies** section above.

**Handling empty `file_paths` on intermediate nodes.** The tag tree
hierarchy (Genre → Artist → Album → Track) stores `file_paths` only on
leaf (track) nodes — intermediate nodes have `file_paths: vec![]`. The
pruning function handles this by recursion: intermediate nodes survive
if any descendant leaf matches.

```rust
/// Recursively prune a TagTreeNode tree using only the tantivy
/// result set. No label matching is performed — the tantivy query
/// determines which paths match, and we keep any node whose subtree
/// contains at least one matching file.
fn prune_tag_node(
    node: &TagTreeNode,
    matches: &HashSet<PathBuf>,
) -> Option<TagTreeNode> {
    if node.children.is_empty() {
        // Leaf node: keep only if any file path is in the match set.
        if node.file_paths.iter().any(|p| matches.contains(p)) {
            Some(node.clone())
        } else {
            None
        }
    } else {
        // Non-leaf: recursively prune children.
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
            cloned.file_count =
                cloned.children.iter().map(|c| c.file_count).sum();
            Some(cloned)
        }
    }
}
```

### The Tantivy Search Query

The debounced `PerformSearch` message handler builds a query that respects
the current `TextSearchMode` field restriction:

```
Match search_mode:
    All          → multi-field query: creator|album|title|genre|filename|path
    Creator      → single-field query: creator
    Album        → single-field query: album
    Title        → single-field query: title
    Genre        → single-field query: genre
    DirectoryPath → single-field query: path
    TrackFilename → single-field query: filename
```

tantivy's default `SimpleTokenizer` lowercases and splits on whitespace,
so queries like `"prog"` or `"prog rock"` work naturally via
`PhrasePrefixQuery`. For `All` mode, a `BooleanQuery` with
`Occur::Should` OR-combines per-field queries (see **Text Search
Strategies** above). For `DirectoryPath` mode, a `RegexQuery` on the
STRING path field handles substring matching.

**Key design rule:** The search mode field restriction is enforced by
tantivy, not by the tree pruning logic. The resulting
`HashSet<PathBuf>` contains only files where the specified field(s)
match the query. The tree pruning code is purely a visual concern
("show only branches containing these paths") and does not re-check
which field matched. This is true regardless of which tree is currently
displayed (directory tree, genre tag tree, or creator tag tree).

### Text Search Strategies

**Field types.** Metadata fields (genre, creator, album, title) use
tantivy's `TEXT` option, which enables tokenization, position indexing,
and frequency scoring. The file path is stored as `STRING` (raw text,
not tokenized) so it can be retrieved exactly and searched with regex.
A separate `filename` field is indexed as `TEXT` for filename searches.

**Tokenizer.** The index uses a `TextAnalyzer` with
`SimpleTokenizer` (splits on whitespace) + `LowerCaser` filter
(produces lowercase tokens). "Progressive Rock" →
`["progressive", "rock"]`. "The Beatles" → `["the", "beatles"]`.
No stop-word removal — common words like "the" are indexed normally to
support searches for "The Beatles".

**Query strategy: `PhrasePrefixQuery` (Recommended).**
`PhrasePrefixQuery` matches phrases where the last term is treated as
a **prefix**. This handles the most common music-search patterns:

| User types | Query built | Matches |
|---|---|---|
| `rock` | `PhrasePrefixQuery(["rock"])` | "Rock", "Progressive Rock" |
| `prog` | `PhrasePrefixQuery(["prog"])` | "Progressive" (prefix of token) |
| `miles dav` | `PhrasePrefixQuery(["miles", "dav"])` | "Miles Davis" (prefix on last token) |
| `dark side` | `PhrasePrefixQuery(["dark", "side"])` | "The Dark Side of the Moon" |

This covers whole-word matching, word-prefix matching, and multi-word
phrases — the vast majority of real-world search patterns. It adds zero
index-size overhead (unlike n-grams) and is significantly faster than
regex.

**Path searches use `RegexQuery`.** Since the path field is `STRING`
(not tokenized), substring matching requires `RegexQuery`:
`".*music/pf.*"` or `".*beatles.*"`. Path regex operates on one raw
string per document, so it's still fast — the regex engine only runs
against the path field, not the full text index.

**Fuzzy fallback for short queries.** When a `PhrasePrefixQuery`
returns zero results and the query is ≤ 3 characters, retry with
`FuzzyTermQuery` (distance 1). This handles typos like `"ruock"` →
`"rock"` and `"jz"` → `"jazz"`. Fuzzy queries are more expensive than
prefix queries, so they're only used as a secondary strategy for
short inputs.

**`All` Mode Query Construction (Settled).** `All` mode builds a
`BooleanQuery` with `Occur::Should` (OR) across all six fields. For
each TEXT field (genre, creator, album, title, filename), a
`PhrasePrefixQuery` is built from the whitespace-split tokens. For the
STRING path field, a `RegexQuery` using `.*query.*`. Each sub-query
operates independently on its field; the `BooleanQuery` OR-combines
the results. This replicates the current behaviour (each field checked
with `.contains()`) at 1.4–3.0ms per All-mode query in release mode
with 16k documents.

**Why not a concatenated super-field.** A single `search_text`
TEXT field concatenating all metadata would: (a) duplicate storage
~2–4× per document, (b) lose per-field query typing (fields have
different query strategies — PhrasePrefix for TEXT, Regex for STRING),
and (c) produce lower-quality results because the same token appears
in multiple contexts. BooleanQuery with per-field sub-queries is the
standard approach and has zero storage overhead.

**What is lost vs. current `.contains()` behaviour.** The current
filter uses case-insensitive substring matching on raw strings.
A query for `"ive"` matches `"Progressive"` because `"ive"` is a
substring. With token-based search, `"ive"` does not match
`"Progressive"` — they are different tokens. However:

- Token-prefix matching covers the common cases ("prog" →
  "Progressive", "dav" → "Davis", "beat" → "Beatles")
- Arbitrary substring matching ("ive") is extremely rare in music
  searches and does not justify the index-size or performance cost of
  n-grams
- Users accustomed to search engines (Spotify, Apple Music) already
  expect word-level and prefix matching, not arbitrary substrings

## Text Search × Left Panel Controls

This section documents how the text search should interact with other
left panel controls, and whether the current or proposed code correctly
handles each interaction.

### File Extensions (AND filter)

The "File Extensions" toggle and the text search act as a **conjunction
(AND)**. A track is displayed in the left panel only if:

1. Its file extension is in the set of selected extensions, **AND**
2. Its metadata or path matches the text search query.

**Current behaviour:** Already AND. `ToggleExtension` rebuilds the
`root_nodes` tree via `scan_directory` (which only includes selected
extensions), then calls `recompute_filtered_nodes` which applies the
text filter on top.

**Proposed behaviour:** Same semantics. When extensions change, the
tantivy index is rebuilt to include only files matching the selected
extensions. The text search then queries that index. The result is
still AND.

### "Select by" (Rearranges the view, not a filter)

The "Select by" button cycles through Directory → GenreTag → CreatorTag.
This changes which tree view is displayed (file tree or tag tree). It
does NOT filter tracks — it rearranges them into a different hierarchy.

**Correct behaviour:**

- When a text search is active and you switch select modes, the search
  filter should still apply to the new mode's tree.
- Example: type "Prog" in Directory mode, switch to GenreTag mode.
  You should see a genre tree pruned to branches containing
  "Progressive Rock" tracks.

**Current code has a bug here:**
`ToggleLeftPanelSelectMode` builds the new `tag_tree_roots` for the
target mode but does NOT re-run the tag tree filter
(`recompute_filtered_tag_nodes`). So `filtered_tag_tree_roots` still
contains stale results from the previous mode's tag tree. The user sees
incorrect/no results.

**Proposed fix (settled):** After building the new `tag_tree_roots` in
`ToggleLeftPanelSelectMode`, re-run both `prune_file_tree` and
`prune_tag_tree` (if a search is active). This is cheap because the
`HashSet<PathBuf>` of matching paths from the last search is stored on
`FileTreeApp` — we just need to re-prune the tree structures against it.
This also fixes a pre-existing bug where switching modes left stale
filtered results.

### "Sort" (Rearranges within the tree, not a filter)

The Sort button cycles Alphanumeric → ModifiedDate → FileCount. This
sorts the children of each tree node within their parent. It acts on
whatever tree is currently displayed (filtered or unfiltered).

**Correct behaviour:**

- Sort should always rearrange the currently displayed tree, whether
  it's the filtered search results or the full unfiltered tree.

**Current behaviour:** Already correct. `render_file_node` and
`render_tag_node` receive `app.left_panel_sort_mode` and sort children
at render time. They operate on whatever tree they're given
(`filtered_root_nodes` or `root_nodes` / `filtered_tag_tree_roots` or
`tag_tree_roots`). No changes needed.

**Proposed behaviour:** Unchanged. The sort mode parameter flows the
same way through the rendering pipeline.

## Model and Message Changes

### New fields on `FileTreeApp`

| Field | Type | Purpose |
|---|---|---|
| `tantivy_index` | `Option<TantivyIndexWrapper>` | In-memory tantivy index (built at startup, rebuilt on file-set changes) |
| `search_generation` | `u64` | Monotonically incrementing counter; discards stale `PerformSearch` results |
| `last_search_matches` | `Option<HashSet<PathBuf>>` | Cached result from the most recent search; enables re-pruning when switching select modes without re-querying tantivy |

All three fields are `#[serde(skip)]`.

### New message variants

| Variant | Payload | Purpose |
|---|---|---|
| `PerformSearch` | `u64` | Runs the tantivy query and prunes both trees; the `u64` payload is the generation that produced this search |

### `TantivyIndexWrapper` API

```rust
/// Wraps an in-memory tantivy index + reader. Must be Send + Sync
/// (tantivy's Index and IndexReader both implement these traits,
/// so this is verified at compile time).
pub(crate) struct TantivyIndexWrapper {
    index: tantivy::Index,
    reader: tantivy::IndexReader,
    schema: tantivy::schema::Schema,
}

impl TantivyIndexWrapper {
    /// Search the index with the given query string and mode.
    /// Returns the set of matching file paths.
    ///
    /// If the query is empty, returns an empty set (caller should
    /// short-circuit and show the full unfiltered tree).
    ///
    /// For TEXT fields (genre, creator, album, title, filename),
    /// builds a `PhrasePrefixQuery` from the whitespace-split tokens.
    /// For the STRING path field, uses `RegexQuery` for substring
    /// matching. `All` mode OR-combines across all fields.
    pub(crate) fn search(
        &self,
        query: &str,
        mode: TextSearchMode,
    ) -> HashSet<PathBuf>;
}
```

**Fallback when the index is `None`:** If `tantivy_index` is `None` when
`PerformSearch` fires, fall back to the current linear-scan functions
(`recompute_filtered_nodes`, `recompute_filtered_tag_nodes`). This ensures
the search bar works even during the brief window before the first index
build completes.

## Dependencies

Add to `Cargo.toml`:

```toml
tantivy = { version = "0.22" }
```

Use tantivy's default features (no mmap required — we use `Index::create_in_ram()`
for an in-memory index). `TermQuery`, `PhrasePrefixQuery`,
`FuzzyTermQuery`, `RegexQuery`, and `BooleanQuery` are all available
without feature flags. Likewise `SimpleTokenizer`, `LowerCaser` filter,
and `TextAnalyzer` are part of the core crate.

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
| `test_prune_tag_partial` | Genre → Artist → Album → Track, only 1 track path in set | Only the matching chain preserved |
| `test_prune_tag_file_count_updated` | Genre with 42 tracks, 3 match | Filtered genre shows `file_count = 3` |

**Debounce / generation counter**

| Test | Input | Expected |
|---|---|---|
| `test_debounce_stale_result_discarded` | Fire `PerformSearch(1)` when `search_generation = 2` | No state change |
| `test_debounce_latest_result_applied` | Fire `PerformSearch(2)` when `search_generation = 2` | Trees pruned |

### Integration Tests

| Test | What it verifies |
|---|---|
| `test_full_search_flow` | Build index with known files → query → verify both pruned trees contain expected paths |
| `test_index_rebuild_on_extension_toggle` | Toggle extension → index rebuilt → search on new extension set works |
| `test_select_mode_switch_preserves_search` | Search in Directory mode → switch to GenreTag → filtered tag tree still shows matching tracks |

### Property-Based Tests

| Test | Invariant |
|---|---|
| `test_index_search_roundtrip` | For any file in `root_nodes` with metadata field value V, searching for V in the corresponding mode returns that file's path |
| `test_prune_idempotency` | Pruning an already-pruned tree produces the same result |

## Trade-Offs

| Aspect | Benefit | Cost |
|---|---|---|
| **Search speed** | O(1) index lookup replaces O(n) disk reads per search | 200–400ms one-time index rebuild on startup and directory/extension changes |
| **Memory** | Fast in-memory queries | Tantivy index overhead: estimated ~10–30 MB for 32k documents with 6 text fields |
| **Compile time** | Default features include mmap; we use in-memory index only | Still adds ~30–60s to clean builds (tantivy is a large crate) |
| **Binary size** | — | Estimated ~2–5 MB increase |
| **Code complexity** | Pruning functions are simpler than current filter functions (no metadata extraction, no string matching) | Schema + query construction code adds a new abstraction layer |

## Open Questions

These questions remain open and should be resolved before or during
implementation. Questions #1, #3, #4, #7, and #8 from the original plan
and the tokenizer + All-mode questions have been settled and moved into
the main document.

### 1. What about the right panel search?

The right panel currently has its own search filter
(`recompute_filtered_right_panel_files`) that reads from
`app.right_panel_files` — an in-memory `Vec<RightPanelFile>` of tracks
the user has manually added. This is a separate, typically much smaller
set (dozens to hundreds of tracks, not 32k).

Should the right panel search also use the tantivy index?

- **Use tantivy** — consistent, but tantivy operates on the full
  visible file set, not the subset the user added to the right panel.
- **Keep current** — the right panel Vec is typically small enough
  that linear scan is fast. Only 32k+ right-panel entries would need it.

### 2. Does tantivy's stored `path` field cause performance issues for large result sets?

We store the full file path in tantivy so we can identify which files
matched. A tantivy query returns `Vec<PathBuf>` of matching files,
which becomes the `HashSet<PathBuf>` for tree pruning. The path is
indexed as text so it can also be searched in `DirectoryPath` mode.

One concern: tantivy's stored fields are deserialised when we iterate
results. For 32k results (empty query = "match everything"), iterating
all 32k stored documents could be slow if done naively. We should:

- Skip the tantivy query entirely when the query is empty (return the
  full unfiltered trees directly, as the code already does).
- For non-empty queries, only iterate the matching subset (typically
  much smaller than 32k).

## Next Steps

1. **Resolve remaining open questions** (#1–#2 above) before implementation.
2. **Create a TODO.md** implementation plan using the `write-todo-from-plan`
   skill, breaking this design into small, testable, commit-sized steps.
3. **Implement incrementally** following the `incremental-development` skill:
   one step at a time, with tests passing and a conventional commit at each
   step.
