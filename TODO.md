# Implementation Plan: Tantivy Text Search Integration

Source: `docs/research/tantivy-integration-research.md`

## Pre-implementation notes

- **`Clone` on `FileTreeApp`**: The model derives `Clone`, and tests use
  `app.clone()` (e.g., `view.rs:486`). `TantivyIndexWrapper` must also be
  `Clone`. In tantivy 0.23, `Index`, `IndexReader`, and `Schema` all use
  internal `Arc` wrapping and implement `Clone` — derive or implement
  `Clone` on `TantivyIndexWrapper` with `#[derive(Clone)]`.
- **Fallback path**: When `tantivy_index` is `None`, `perform_search()`
  returns empty — the old `filter_file_node`/`filter_tag_node` functions
  are kept as dead code for this fallback and are never removed.
- **No `PerformSearch` message**: The research doc's message table lists
  `PerformSearch(u64)` but the final handler design calls
  `app.perform_search()` directly — no message variant is needed.

| # | Commit message | Logical unit | Key deliverables | Tests |
|---|---|---|---|---|
| 1 | `chore: add tantivy dependency and TantivyIndexWrapper` | TantivyIndexWrapper + search | `Cargo.toml`, `src/gui/tantivy_search.rs` | Unit |
| 2 | `feat: implement tantivy index builder and tree pruners` | build_tantivy_index + prune functions | `src/gui/tantivy_search.rs` | Unit |
| 3 | `feat: add tantivy model fields and perform_search` | Model integration | `src/gui/state.rs`, `src/gui/tantivy_search.rs` | Unit |
| 4 | `feat: wire tantivy into all message handlers` | Handler integration | `src/gui/update.rs` | Unit, Integration |
| 5 | `test: add integration and property-based tests for tantivy search flow` | Integration + property tests | `src/gui/tantivy_search.rs`, `src/gui/update.rs` | Integration, Property |

---

### Step 1 — TantivyIndexWrapper + search

**Add tantivy to `Cargo.toml`:**

```toml
tantivy = "0.23"
```

**Create `src/gui/tantivy_search.rs`** with a module-level docstring.

**Define the schema** (6 fields):

| Tantivy field | Type | Indexed | Stored | Purpose |
|---|---|---|---|---|
| `path` | STRING | Yes | Yes | Identify matches; prune trees |
| `filename` | TEXT | Yes | No | `TrackFilename` mode |
| `creator` | TEXT | Yes | No | `Creator` / `All` mode |
| `album` | TEXT | Yes | No | `Album` / `All` mode |
| `title` | TEXT | Yes | No | `Title` / `All` mode |
| `genre` | TEXT | Yes | No | `Genre` / `All` mode |

Tokenizer: `SimpleTokenizer` + `LowerCaser`. No stop-word removal.

**Define `TantiyIndexWrapper`:**

```rust
#[derive(Clone)]
pub(crate) struct TantivyIndexWrapper {
    index: tantivy::Index,
    reader: tantivy::IndexReader,
    schema: tantivy::schema::Schema,
}
```

Implement `Debug` (showing `num_docs` and schema fields).

Implement `TantivyIndexWrapper::search()`:

```rust
pub(crate) fn search(
    &self,
    query: &str,
    mode: TextSearchMode,
) -> Result<HashSet<PathBuf>, tantivy::TantivyError>
```

Query strategy:

- **TEXT fields** (`creator`, `album`, `title`, `genre`, `filename`):
  `PhrasePrefixQuery` (last term is a prefix).
- **STRING field** (`path`): `RegexQuery` with pattern `.*<query>.*`.
- **All mode**: `BooleanQuery` with `Occur::Should` across all 6 fields.
- **Fuzzy fallback**: When `PhrasePrefixQuery` returns 0 results and the
  query is ≤ 3 characters, retry with `FuzzyTermQuery` (distance 1).

**Wire the module** by adding `pub mod tantivy_search;` to `src/gui/mod.rs`.

**Unit tests:**

| Test | Input | Expected |
|---|---|---|
| `test_search_empty_index` | Empty index, any query | Empty `HashSet` |
| `test_search_single_doc_by_title` | 1 doc with title "Test Song", query "test" | Path of that doc |
| `test_search_single_doc_by_creator` | 1 doc with creator "Artist", query "artist" | Path of that doc |
| `test_search_all_mode_multi_field` | 2 docs, query matches different fields | Both paths |
| `test_search_path_regex` | Doc at "/music/jazz/track.mp3", Path mode "jazz" | Path |
| `test_search_filename_prefix` | Doc named "progressive_rock.mp3", query "prog" | Path |
| `test_search_no_match` | 1 doc, query "nonexistent" | Empty |
| `test_search_fuzzy_fallback` | Doc "rock", query "ruock" (3 chars or less) | Path |
| `test_search_fuzzy_boundary_4chars` | Doc "rock", query "rockk" (4 chars) | Empty (no fuzzy fallback) |
| `test_search_debug_format` | Wrapper with 1 doc | Debug shows num_docs |

---

### Step 2 — Index builder and tree pruners

**Add to `src/gui/tantivy_search.rs`:**

**`build_tantivy_index`:**

```rust
pub(crate) fn build_tantivy_index(
    root_nodes: &[Option<FileNode>],
) -> TantivyIndexWrapper
```

- Walks `FileNode` trees recursively.
- For `File` nodes: extracts metadata via `extract_media_metadata(path)`.
- Creates one tantivy document per file with all 6 fields.
- `path` stored as `path.to_string_lossy()`.
- Returns a `TantivyIndexWrapper`.

**`prune_file_tree`:**

```rust
pub(crate) fn prune_file_tree(
    node: &FileNode,
    matches: &HashSet<PathBuf>,
    query: &str,
    mode: TextSearchMode,
) -> Option<FileNode>
```

- Recursively walks the `FileNode` tree.
- For each directory, checks if its path/name matches the query string
  (for `DirectoryPath` and `All` modes — lightweight string comparison).
- Prunes children against the match set.
- Recomputes `file_count` to reflect only visible descendants.
- No disk I/O.

**`prune_tag_node`:**

```rust
pub(crate) fn prune_tag_node(
    node: &TagTreeNode,
    matches: &HashSet<PathBuf>,
) -> Option<TagTreeNode>
```

- Recursively walks the `TagTreeNode` tree.
- Pure `HashSet` lookup — no label matching or string scans.
- Intermediate nodes survive if any descendant leaf matches.
- Recomputes `file_count` to reflect only visible descendants.

**Unit tests:**

| Test | What it verifies |
|---|---|
| `test_build_empty` | Empty `root_nodes` → index with 0 docs |
| `test_build_single_file` | One `FileNode::File` with metadata → index has 1 doc, search by title returns path |
| `test_build_many_files` | 5 files with varied metadata → all docs retrievable |
| `test_build_skips_non_audio` | Mix of audio and non-audio → only audio indexed (FileNode already skips non-audio) |
| `test_prune_file_tree_empty_tree` | `None` node → `None` |
| `test_prune_file_tree_no_matches` | 3-file tree, empty `HashSet` → `None` for root |
| `test_prune_file_tree_all_match` | 3-file tree, all paths in set → all preserved |
| `test_prune_file_tree_partial` | 3-file tree, 1 path in set → only matching file |
| `test_prune_file_tree_file_count_updated` | Dir with 3 children, 1 match → `file_count = 1` |
| `test_prune_file_tree_directory_name_match` | Dir with name matching query, no child matches → dir kept with `file_count = 0` |
| `test_prune_tag_node_empty` | Empty `HashSet` → `None` |
| `test_prune_tag_node_all_match` | All leaf paths in set → full tree preserved |
| `test_prune_tag_node_partial` | Genre→Artist→Album→Track chain, 1 match → only matching chain preserved |
| `test_prune_tag_node_file_count_updated` | Genre with 42 tracks, 3 match → `file_count = 3` |
| `test_prune_tag_node_intermediate_empty_paths` | Node with empty `file_paths` but matching child → kept |

---

### Step 3 — Model fields + `perform_search`

**Modify `src/gui/state.rs`:**

Add to `FileTreeApp`:

```rust
#[serde(skip)]
pub tantivy_index: Option<TantivyIndexWrapper>,
#[serde(skip)]
pub search_generation: u64,
#[serde(skip)]
pub last_search_matches: Option<HashSet<PathBuf>>,
```

**Add `perform_search()` method to `FileTreeApp`:**

```rust
impl FileTreeApp {
    pub(crate) fn perform_search(&mut self) {
        // Increment generation, run tantivy search, prune both trees,
        // filter right panel files via HashSet lookup.
        // Guard: discard stale results if gen changed.
    }
}
```

**Modify `FileTreeApp::new()`** to build the initial index after `root_nodes` are built:

```rust
self.tantivy_index = Some(build_tantivy_index(&root_nodes));
```

**Import the new module** in `state.rs` and `update.rs`:

```rust
use crate::gui::tantivy_search::{TantivyIndexWrapper, build_tantivy_index,
    prune_file_tree, prune_tag_node};
```

**Unit tests on `FileTreeApp`:**

| Test | Input | Expected |
|---|---|---|
| `test_new_app_has_index` | `new()` with empty dirs | `tantivy_index` is `Some` |
| `test_perform_search_empty_query` | Empty query | `last_search_matches` is `None`, filtered trees are clones |
| `test_perform_search_with_query` | Non-empty query matching known path | `last_search_matches` is `Some` with that path |
| `test_perform_search_no_match` | Query not matching any doc | `last_search_matches` is `Some(HashSet::new())`, all filtered trees empty |
| `test_generation_stale_result_discarded` | Increment gen between search calls | Stale result not applied |
| `test_generation_latest_applied` | Same gen throughout | Search applied normally |
| `test_tantivy_index_serde_skip` | Serialize + deserialize | Index is None after round-trip |

---

### Step 4 — Wire message handlers

**Modify `src/gui/update.rs`:**

The following handlers change from calling `recompute_filtered_*` to
calling `app.perform_search()` or directly clearing state:

1. **`SearchQueryChanged`** — Replace 3 `recompute` calls with:
   - If query empty: clear `last_search_matches`, clone unfiltered trees.
   - Else: call `app.perform_search()`.

2. **`SearchCleared`** — Replace 3 `recompute` calls with:
   - Clear `last_search_matches`, clone unfiltered trees.
   - (Same as SearchQueryChanged empty query path.)

3. **`ToggleSearchMode`** — Replace 3 `recompute` calls with:
   - If query non-empty: call `app.perform_search()`.
   - If query empty: no-op (no filtered trees to update).

4. **`ToggleLeftPanelSelectMode`** — After rebuilding `tag_tree_roots`:
   - If query non-empty and `last_search_matches` is `Some`: re-prune
     `filtered_tag_tree_roots` against cached match set.
   - If query non-empty and no cache: call `app.perform_search()`.
   - Fixes the pre-existing bug where switching modes left stale
     `filtered_tag_tree_roots`.

5. **`ToggleExtension`** — After rebuilding `root_nodes`:
   - Add: `app.tantivy_index = Some(build_tantivy_index(&app.root_nodes))`.
   - Replace `recompute_filtered_nodes` + `recompute_filtered_tag_nodes`
     with: if query active, call `app.perform_search()` (after rebuilding tag
     tree roots too).

6. **`AddDirectory`** — No change (returns a `Task`).
   **`DirectoryAdded`** — After adding the new directory to `root_nodes`:
   - Add: rebuild index + re-apply search if query active.

7. **`RemoveTopDir`** — After removing the directory from `root_nodes`:
   - Add: rebuild index + re-apply search if query active.

**Keep the old helper functions** (`recompute_filtered_nodes`,
`recompute_filtered_tag_nodes`, `recompute_filtered_right_panel_files`,
`filter_file_node`, `filter_tag_node`, `file_matches_mode`) for the
fallback path. They become dead code but serve as safety net.

**Existing tests should pass** — the behavior is preserved for the case
where no tantivy index exists (fallback). New tests cover the tantivy
path.

**Unit tests:**

| Test | Input | Expected |
|---|---|---|
| `test_search_query_changed_tantivy` | Query matching known path | filtered trees contain that path |
| `test_search_query_changed_no_match` | Query not matching any path | filtered trees are empty |
| `test_search_cleared_tantivy` | Active search, then SearchCleared | unfiltered trees restored |
| `test_toggle_search_mode_tantivy` | Active search, toggle mode | filtered trees updated for new mode |
| `test_toggle_left_panel_select_mode_bug_fix` | Search active, switch mode | filtered_tag_tree_roots correctly pruned |

---

### Step 5 — Integration + property tests

**Add to `src/gui/tantivy_search.rs` or `src/gui/update.rs`:**

**Integration tests:**

| Test | What it verifies |
|---|---|
| `test_full_search_flow` | Build index with known files → query → verify both pruned trees contain expected paths |
| `test_index_rebuild_on_extension_toggle` | Toggle extension → index rebuilt → search on new extension set works |
| `test_select_mode_switch_preserves_search` | Search in Directory mode → switch to GenreTag → filtered tag tree still shows matching tracks |
| `test_toggle_left_panel_select_mode_during_search` | Switch between Dir/Genre/Creator while search active → each mode shows correctly filtered results |

**Property-based tests:**

| Test | Invariant |
|---|---|
| `test_index_search_roundtrip` | For any file with metadata V, searching for V in the corresponding mode returns that file |
| `test_index_search_roundtrip_unicode` | Same with unicode metadata (artists like "Mø", "Björk") to verify unicode tokenization |
| `test_prune_idempotency` | Pruning an already-pruned tree produces the same result |
| `test_fuzzy_fallback_typo` | Query "ruock" with 0 direct results triggers fuzzy fallback and returns "rock" (distance 1) |
| `test_fuzzy_fallback_boundary_4chars` | Query "rockk" (4 chars) with 0 direct results does NOT trigger fuzzy fallback |
| `test_fuzzy_fallback_no_false_positive` | Short query with no close match returns empty even after fuzzy fallback |
